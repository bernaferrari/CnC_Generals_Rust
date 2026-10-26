//! Receiving Diplomacy.cpp owner and synchronous native-world operations.
use super::*;
use crate::core::game_client::ClientInputAuthority;
use crate::gui::game_window::WindowEventEffects;
use crate::gui::window_manager::WindowManager;

#[derive(Clone, Debug)]
pub struct DiplomacyWorldFacts {
    pub input_enabled: bool,
    pub intro_movie: bool,
    pub loading_map: bool,
    pub multiplayer: bool,
    pub local_slot: Option<i32>,
    pub players: HashMap<i32, PlayerInfo>,
}
fn error(message: impl Into<String>) -> Box<dyn std::error::Error> {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message.into()).into()
}
fn world(
    effects: &mut WindowEventEffects<'_>,
) -> Result<DiplomacyWorldFacts, Box<dyn std::error::Error>> {
    if effects.input_authority() == ClientInputAuthority::NativeHosted {
        return effects
            .diplomacy_world
            .cloned()
            .ok_or_else(|| error("native Diplomacy requires its receiving world observations"));
    }
    // Only the separately retained StandaloneCore runtime uses this source.
    let list = ThePlayerList()
        .read()
        .map_err(|_| error("Core player list unavailable"))?;
    let mut players = HashMap::new();
    let mut local_slot = None;
    let local_index = list
        .get_local_player()
        .and_then(|local| local.read().ok().map(|local| local.get_player_index()));
    for slot in 0..MAX_SLOTS {
        let Some(player) = list.find_player_by_name(&format!("player{slot}")) else {
            continue;
        };
        let player = player
            .read()
            .map_err(|_| error("Core player unavailable"))?;
        let mut info = PlayerInfo::default();
        info.team = player
            .get_default_team_id()
            .map(|id| id as i32)
            .unwrap_or(-1);
        info.status = if player.is_player_active() {
            PlayerStatus::Active
        } else {
            PlayerStatus::Observer
        };
        info.side = player.get_side().into();
        info.name = NameKeyGenerator::key_to_name(player.get_player_name_key())
            .unwrap_or_else(|| format!("Player {}", slot + 1));
        if local_index == Some(player.get_player_index()) {
            local_slot = Some(slot as i32);
        }
        players.insert(slot as i32, info);
    }
    Ok(DiplomacyWorldFacts {
        input_enabled: TheGameLogic::is_input_enabled(),
        intro_movie: TheGameLogic::is_intro_movie_playing(),
        loading_map: TheGameLogic::is_loading_map(),
        multiplayer: with_recorder(|r| r.is_multiplayer())
            .ok_or_else(|| error("Core recorder unavailable"))?,
        local_slot,
        players,
    })
}
fn admit(facts: DiplomacyWorldFacts, effects: &mut WindowEventEffects<'_>) {
    effects.with_diplomacy(|owner| {
        owner.local_player_id = facts.local_slot.unwrap_or(-1);
        owner.players = facts.players;
    });
}
fn main_window(effects: &mut WindowEventEffects<'_>) -> Option<Rc<RefCell<GameWindow>>> {
    effects.with_diplomacy(|o| o.ui.window.clone())
}
fn with_window<R>(
    target: &Rc<RefCell<GameWindow>>,
    selected: Option<&mut GameWindow>,
    f: impl FnOnce(&mut GameWindow) -> R,
) -> R {
    if let Some(selected) = selected
        && std::ptr::eq(target.as_ptr(), std::ptr::from_ref(&*selected))
    {
        return f(selected);
    }
    f(&mut target.borrow_mut())
}
fn canonical(window: &GameWindow, effects: &mut WindowEventEffects<'_>) -> bool {
    let pointer = std::ptr::from_ref(window);
    effects.with_manager_ref(|m| {
        m.get_window_by_id(window.get_id())
            .is_some_and(|w| std::ptr::eq(w.as_ptr(), pointer))
    })
}
pub fn toggle_diplomacy(
    immediate: bool,
    manager: &mut WindowManager,
    effects: &mut WindowEventEffects<'_>,
    mut selected: Option<&mut GameWindow>,
) -> Result<(), Box<dyn std::error::Error>> {
    super::super::quit_menu::hide_quit_menu(manager, selected.as_deref_mut(), effects);
    let show = main_window(effects)
        .is_none_or(|window| with_window(&window, selected.as_deref_mut(), |w| w.is_hidden()));
    if show {
        show_diplomacy(immediate, manager, effects, selected)
    } else {
        hide_selected(immediate, manager, effects, selected)
    }
}
fn show_diplomacy(
    immediate: bool,
    manager: &mut WindowManager,
    effects: &mut WindowEventEffects<'_>,
    mut selected: Option<&mut GameWindow>,
) -> Result<(), Box<dyn std::error::Error>> {
    let facts = world(effects)?;
    if !facts.input_enabled
        || facts.intro_movie
        || facts.loading_map
        || effects.ui().is_quit_menu_visible()
        || effects.receiving_disconnect_menu_visible()
    {
        return Ok(());
    }
    let multiplayer = facts.multiplayer;
    admit(facts, effects);
    let (layout, window, created) = if let Some(window) = main_window(effects) {
        let layout = effects
            .with_diplomacy(|o| o.ui.layout.clone())
            .ok_or_else(|| error("Diplomacy root has no layout"))?;
        (layout, window, false)
    } else {
        let generation = effects
            .with_diplomacy(|o| if o.retiring { None } else { Some(o.generation) })
            .ok_or_else(|| error("Diplomacy is retiring"))?;
        let (layout, _) = manager.create_layout_with_windows("Diplomacy.wnd", effects)?;
        let window = layout
            .borrow()
            .get_first_window()
            .ok_or_else(|| error("Diplomacy layout has no root"))?;
        let accepted = effects.with_diplomacy(|o| {
            if o.generation != generation || o.retiring || o.ui.layout.is_some() {
                return false;
            }
            o.ui.layout = Some(layout.clone());
            o.ui.window = Some(window.clone());
            true
        });
        if !accepted {
            manager.destroy_layout(&layout, effects);
            return Err(error("Diplomacy layout admission was superseded"));
        }
        (layout, window, true)
    };
    manager.hide_window_with_selected(&window, false, selected.as_deref_mut())?;
    manager.enable_window_with_selected(&window, true, selected.as_deref_mut())?;
    // Layout hide is a property of this exact receiving layout, not a registry.
    layout
        .borrow()
        .hide_with_manager_and_selected(manager, false, selected.as_deref_mut());
    for (name, hidden) in [
        ("RadioButtonInGame", true),
        ("RadioButtonBuddies", true),
        ("InGameParent", !multiplayer),
        ("BuddiesParent", true),
        ("SoloParent", multiplayer),
    ] {
        if let Some(w) = manager.get_window_by_id(NameKeyGenerator::name_to_key(&format!(
            "Diplomacy.wnd:{name}"
        )) as i32)
        {
            manager.hide_window_with_selected(&w, hidden, selected.as_deref_mut())?;
        }
    }
    if created && !multiplayer {
        let labels = effects.with_diplomacy(|o| o.briefing_list.clone());
        for (index, label) in labels.iter().enumerate() {
            DiplomacyCallbacks::append_briefing_to_listbox(label, index, manager);
        }
    }
    effects.with_diplomacy(|o| {
        o.ui.animate_manager.reset();
        if !immediate && get_global_data().is_some_and(|d| d.read().animate_windows) {
            o.ui.animate_manager
                .register_window(window, AnimationType::SlideTop, true, 200, 0);
        }
        o.active = true;
    });
    populate(manager, effects, selected, true)?;
    Ok(())
}
pub fn hide_diplomacy(
    immediate: bool,
    manager: &mut WindowManager,
    effects: &mut WindowEventEffects<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    hide_selected(immediate, manager, effects, None)
}
fn hide_selected(
    immediate: bool,
    manager: &mut WindowManager,
    effects: &mut WindowEventEffects<'_>,
    mut selected: Option<&mut GameWindow>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(window) = main_window(effects) {
        if immediate || !get_global_data().is_some_and(|d| d.read().animate_windows) {
            manager.hide_window_with_selected(&window, true, selected.as_deref_mut())?;
            manager.enable_window_with_selected(&window, false, selected.as_deref_mut())?;
        } else {
            effects.with_diplomacy(|o| {
                if o.ui.animate_manager.is_finished() {
                    o.ui.animate_manager.reverse_animate_window();
                }
            });
        }
    }
    effects.with_diplomacy(|o| o.active = false);
    Ok(())
}
pub fn reset_diplomacy(
    manager: &mut WindowManager,
    effects: &mut WindowEventEffects<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    let phase = effects.with_diplomacy(|o| {
        if o.retiring {
            return None;
        }
        o.retiring = true;
        Some((o.generation, o.ui.layout.clone()))
    });
    let Some((generation, layout)) = phase else {
        return Ok(());
    };
    if let Some(layout) = layout {
        manager.destroy_layout(&layout, effects);
    }
    effects.with_diplomacy(|o| {
        if o.generation == generation {
            o.ui.layout = None;
            o.ui.window = None;
            o.ui.animate_manager.reset();
            o.retiring = false;
            o.generation = generation.wrapping_add(1);
            o.active = false;
        }
    });
    Ok(())
}
pub fn is_diplomacy_active(owner: &DiplomacyCallbacks) -> bool {
    owner
        .ui
        .window
        .as_ref()
        .is_some_and(|w| !w.borrow().is_hidden())
}
pub fn get_briefing_text_list(owner: &DiplomacyCallbacks) -> Vec<String> {
    owner.briefing_list.clone()
}
pub fn update_diplomacy_briefing_text(
    text: &str,
    clear: bool,
    manager: &mut WindowManager,
    owner: &mut DiplomacyCallbacks,
) {
    owner.update_briefing_text(text, clear, manager);
}
pub fn tick_diplomacy_animation(manager: &mut WindowManager, owner: &mut DiplomacyCallbacks) {
    if !get_global_data().is_some_and(|d| d.read().animate_windows) || owner.ui.window.is_none() {
        return;
    }
    let was = owner.ui.animate_manager.is_finished();
    owner.ui.animate_manager.update();
    if owner.ui.animate_manager.is_finished() && !was && owner.ui.animate_manager.is_reversed() {
        if let Some(window) = owner.ui.window.clone() {
            let _ = manager.hide_window_with_selected(&window, true, None);
        }
    }
}
fn populate(
    manager: &mut WindowManager,
    effects: &mut WindowEventEffects<'_>,
    mut selected: Option<&mut GameWindow>,
    refresh: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if refresh {
        let facts = world(effects)?;
        admit(facts, effects);
    }
    let mut players: Vec<_> = effects
        .with_diplomacy(|o| o.players.clone())
        .into_iter()
        .collect();
    players.sort_by_key(|(slot, _)| *slot);
    effects.with_diplomacy(|o| {
        o.ui.slots_in_rows = [None; MAX_SLOTS];
        for (row, (slot, _)) in players.iter().enumerate() {
            o.ui.slots_in_rows[row] = Some(*slot);
        }
    });
    // The receiving data snapshot is immutable while field-only widgets render.
    for (row, (_slot, info)) in players.into_iter().enumerate() {
        let fields = [
            ("Player", info.name),
            ("Side", info.side),
            ("Team", info.team.to_string()),
            ("Status", format!("{:?}", info.status)),
        ];
        for (field, text) in fields {
            if let Some(w) = manager.get_window_by_id(NameKeyGenerator::name_to_key(&format!(
                "Diplomacy.wnd:StaticText{field}{row}"
            )) as i32)
            {
                with_window(&w, selected.as_deref_mut(), |w| {
                    let _ = w.set_text(&text);
                });
            }
        }
    }
    Ok(())
}
pub fn diplomacy_system_receiving(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
    effects: &mut WindowEventEffects<'_>,
) -> WindowMsgHandled {
    if msg == WindowMessage::InputFocus {
        return crate::gui::game_window::write_input_focus_response(data1, _data2, false);
    }
    if msg == WindowMessage::Create || msg == WindowMessage::Destroy {
        return WindowMsgHandled::Handled;
    }
    if !canonical(window, effects) {
        return WindowMsgHandled::Ignored;
    }
    if msg != WindowMessage::GadgetSelected {
        return WindowMsgHandled::Ignored;
    }
    let result = effects.with_manager(
        |manager, effects| -> Result<(), Box<dyn std::error::Error>> {
            let key = data1 as u32;
            if key == NameKeyGenerator::name_to_key("Diplomacy.wnd:ButtonHide") {
                return hide_selected(false, manager, effects, Some(window));
            }
            for (name, hidden) in [
                (
                    "InGameParent",
                    key == NameKeyGenerator::name_to_key("Diplomacy.wnd:RadioButtonBuddies"),
                ),
                (
                    "BuddiesParent",
                    key == NameKeyGenerator::name_to_key("Diplomacy.wnd:RadioButtonInGame"),
                ),
            ] {
                if key == NameKeyGenerator::name_to_key("Diplomacy.wnd:RadioButtonInGame")
                    || key == NameKeyGenerator::name_to_key("Diplomacy.wnd:RadioButtonBuddies")
                {
                    if let Some(w) = manager.get_window_by_id(NameKeyGenerator::name_to_key(
                        &format!("Diplomacy.wnd:{name}"),
                    ) as i32)
                    {
                        manager.hide_window_with_selected(&w, hidden, Some(window))?;
                    }
                }
            }
            for slot in 0..MAX_SLOTS {
                for (prefix, muted) in [("ButtonMute", true), ("ButtonUnMute", false)] {
                    if key
                        == NameKeyGenerator::name_to_key(&format!("Diplomacy.wnd:{prefix}{slot}"))
                    {
                        let Some(actual_slot)=effects.with_diplomacy(|owner|owner.ui.slots_in_rows[slot]) else { return Ok(()) };
                        if effects.input_authority() == ClientInputAuthority::NativeHosted {
                            return Err(error("native Diplomacy muting requires live receiving GameInfo; immutable initial admission cannot be mutated"));
                        } else {
                            effects.with_diplomacy(|o| {
                                let _ = o.set_player_muted(actual_slot, muted, manager);
                            });
                        }
                        return populate(manager, effects, Some(window), false);
                    }
                }
            }
            Ok(())
        },
    );
    match result {
        Ok(()) => WindowMsgHandled::Handled,
        Err(e) => {
            log::error!("receiving Diplomacy callback failed: {e}");
            WindowMsgHandled::Ignored
        }
    }
}
pub fn diplomacy_input_receiving(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
    effects: &mut WindowEventEffects<'_>,
) -> WindowMsgHandled {
    if msg != WindowMessage::Char || !canonical(window, effects) {
        return WindowMsgHandled::Ignored;
    }
    if data1 == KEY_ESC {
        if let Err(e) = effects.with_manager(|m, e| hide_selected(false, m, e, Some(window))) {
            log::error!("receiving Diplomacy hide failed: {e}");
            return WindowMsgHandled::Ignored;
        }
    }
    WindowMsgHandled::Handled
}

#[cfg(test)]
mod tests;
