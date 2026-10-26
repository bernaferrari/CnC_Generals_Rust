//! Instance-owned InGameChat.cpp state and synchronous WND operations.
use super::*;
use crate::core::game_client::ClientInputAuthority;
use crate::gui::game_window::WindowEventEffects;
use crate::gui::window_manager::WindowManager;

/// Original named player0..player7 slot observations, frozen for this operation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChatSlotFacts {
    pub is_local: bool,
    pub mutually_allied: bool,
    pub muted: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChatWorldFacts {
    pub multiplayer: bool,
    pub local_player_active: bool,
    pub local_slot: Option<u8>,
    pub slots: [Option<ChatSlotFacts>; 8],
}
impl ChatWorldFacts {
    pub fn player_mask(&self, chat_type: &InGameChatType) -> i32 {
        self.slots
            .iter()
            .enumerate()
            .fold(0, |mask, (slot, player)| {
                let Some(player) = player else { return mask };
                let include = match chat_type {
                    InGameChatType::Everyone => !player.muted,
                    InGameChatType::Players => player.is_local,
                    InGameChatType::Allies => player.is_local || player.mutually_allied,
                };
                if include { mask | (1 << slot) } else { mask }
            })
    }
}

fn missing_context(message: &'static str) -> Box<dyn std::error::Error> {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message).into()
}
fn chat_world(
    effects: &mut WindowEventEffects<'_>,
) -> Result<ChatWorldFacts, Box<dyn std::error::Error>> {
    if effects.input_authority() == ClientInputAuthority::NativeHosted {
        return effects
            .menu_world
            .and_then(|world| world.chat)
            .ok_or_else(|| {
                missing_context("native chat requires its receiving player observations")
            });
    }
    // Explicitly bounded old Core adapter. Native never discovers Core players.
    // Passing an owned Core roster remains a separate Core-world source migration.
    let list = ThePlayerList()
        .read()
        .map_err(|_| missing_context("Core player owner is unavailable"))?;
    let local = list
        .get_local_player()
        .cloned()
        .ok_or_else(|| missing_context("Core chat requires its local player"))?;
    let local = local
        .read()
        .map_err(|_| missing_context("Core local player is unavailable"))?;
    let mut facts = ChatWorldFacts {
        multiplayer: TheGameLogic::is_in_multiplayer_game(),
        local_player_active: local.is_player_active(),
        local_slot: None,
        slots: [None; 8],
    };
    for slot in 0..8 {
        let Some(player) = list.find_player_by_name(&format!("player{slot}")) else {
            continue;
        };
        let player = player
            .read()
            .map_err(|_| missing_context("Core chat player is unavailable"))?;
        let is_local = player.get_player_index() == local.get_player_index();
        if is_local {
            facts.local_slot = Some(slot as u8);
        }
        let mutually_allied = if is_local {
            true
        } else if let (Some(other_team), Some(local_team)) =
            (player.get_default_team(), local.get_default_team())
        {
            let other_team = other_team
                .read()
                .map_err(|_| missing_context("Core player team is unavailable"))?;
            let local_team = local_team
                .read()
                .map_err(|_| missing_context("Core local team is unavailable"))?;
            player.get_relationship_with_team(&local_team) == Relationship::Allies
                && local.get_relationship_with_team(&other_team) == Relationship::Allies
        } else {
            false
        };
        // Core's receiving GameInfo/slot mute observation remains with that
        // explicit legacy role; native facts use its saved GameInfo admission.
        facts.slots[slot] = Some(ChatSlotFacts {
            is_local,
            mutually_allied,
            muted: false,
        });
    }
    Ok(facts)
}
fn should_block_chat(effects: &WindowEventEffects<'_>) -> Result<bool, Box<dyn std::error::Error>> {
    let replay = match effects.input_authority() {
        ClientInputAuthority::NativeHosted => {
            effects
                .menu_world
                .ok_or_else(|| missing_context("native chat requires world facts"))?
                .game_mode
                == gamelogic::system::game_logic::GAME_REPLAY
        }
        ClientInputAuthority::StandaloneCore => TheGameLogic::is_in_replay_game(),
    };
    Ok(
        replay
            || effects.ui().is_quit_menu_visible()
            || effects.receiving_disconnect_menu_visible(),
    )
}
fn with_window<R>(
    target: &Rc<RefCell<GameWindow>>,
    selected: Option<&mut GameWindow>,
    operation: impl FnOnce(&mut GameWindow) -> R,
) -> R {
    if let Some(selected) = selected
        && std::ptr::eq(target.as_ptr(), std::ptr::from_ref(&*selected))
    {
        return operation(selected);
    }
    operation(&mut target.borrow_mut())
}
fn entry_text(target: &Rc<RefCell<GameWindow>>, selected: Option<&mut GameWindow>) -> String {
    with_window(target, selected, |window| {
        window
            .text_entry_mut()
            .map(|entry| entry.text().to_owned())
            .unwrap_or_default()
    })
}
fn set_entry_text(target: &Rc<RefCell<GameWindow>>, text: &str, selected: Option<&mut GameWindow>) {
    with_window(target, selected, |window| {
        if let Some(entry) = window.text_entry_mut() {
            entry.set_text(text);
        }
    });
}
fn windows(effects: &mut WindowEventEffects<'_>) -> Option<ChatWindows> {
    effects.with_ingame_ui(|owner| owner.chat.ui.phase.windows().cloned())
}
fn live_callback(window: &GameWindow, effects: &mut WindowEventEffects<'_>) -> bool {
    let pointer = std::ptr::from_ref(window);
    let registered = effects.with_manager_ref(|manager| {
        manager
            .get_window_by_id(window.get_id())
            .is_some_and(|stored| std::ptr::eq(stored.as_ptr(), pointer))
    });
    registered
        && effects.with_ingame_ui(|owner| {
            owner.chat.ui.phase.windows().is_some_and(|windows| {
                [
                    &windows.parent,
                    &windows.text_entry,
                    &windows.chat_type_text,
                ]
                .into_iter()
                .any(|stored| std::ptr::eq(stored.as_ptr(), pointer))
            })
        })
}
fn ensure_layout(
    manager: &mut WindowManager,
    effects: &mut WindowEventEffects<'_>,
) -> Result<ChatWindows, Box<dyn std::error::Error>> {
    if let Some(existing) = windows(effects) {
        if effects
            .with_ingame_ui(|owner| matches!(owner.chat.ui.phase, ChatLayoutPhase::Retiring(_)))
        {
            return Err(missing_context("chat layout is retiring"));
        }
        return Ok(existing);
    }
    let generation = effects.with_ingame_ui(|owner| owner.chat.ui.generation);
    let (layout, _) = manager.create_layout_with_windows("InGameChat.wnd", effects)?;
    let lookup = |name| manager.get_window_by_id(NameKeyGenerator::name_to_key(name) as i32);
    let required = (
        lookup("InGameChat.wnd:ParentInGameChat"),
        lookup("InGameChat.wnd:TextEntryChat"),
        lookup("InGameChat.wnd:StaticTextChatType"),
    );
    let (Some(parent), Some(text_entry), Some(chat_type_text)) = required else {
        manager.destroy_layout(&layout, effects);
        return Err(missing_context(
            "authored chat layout is missing required controls",
        ));
    };
    set_entry_text(&text_entry, "", None);
    let candidate = ChatWindows {
        layout,
        parent,
        text_entry,
        chat_type_text,
    };
    let admitted = effects.with_ingame_ui(|owner| {
        if owner.chat.ui.generation != generation
            || !matches!(owner.chat.ui.phase, ChatLayoutPhase::Absent)
        {
            return false;
        }
        owner.chat.ui.phase = ChatLayoutPhase::Live(candidate.clone());
        true
    });
    if !admitted {
        manager.destroy_layout(&candidate.layout, effects);
        return Err(missing_context(
            "chat owner changed while its layout was being created",
        ));
    }
    Ok(candidate)
}
pub(crate) fn chat_type_label(chat_type: &InGameChatType, active: bool) -> String {
    GameText::fetch(match chat_type {
        InGameChatType::Everyone if active => "Chat:Everyone",
        InGameChatType::Everyone => "Chat:Observers",
        InGameChatType::Allies => "Chat:Allies",
        InGameChatType::Players => "Chat:Players",
    })
}
pub fn set_in_game_chat_type(
    chat_type: InGameChatType,
    effects: &mut WindowEventEffects<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    set_type_selected(chat_type, effects, None)
}
fn set_type_selected(
    chat_type: InGameChatType,
    effects: &mut WindowEventEffects<'_>,
    selected: Option<&mut GameWindow>,
) -> Result<(), Box<dyn std::error::Error>> {
    let label = effects.with_ingame_ui(|owner| owner.assign_chat_type(chat_type.clone()));
    if let Some(label) = label {
        let active = chat_world(effects)?.local_player_active;
        let text = chat_type_label(&chat_type, active);
        with_window(&label, selected, |label| {
            let _ = label.set_text(&text);
        });
    }
    Ok(())
}
pub fn get_in_game_chat_type(effects: &mut WindowEventEffects<'_>) -> InGameChatType {
    effects.with_ingame_ui(|owner| owner.chat.chat_type.clone())
}
pub fn is_in_game_chat_active(effects: &mut WindowEventEffects<'_>) -> bool {
    windows(effects).is_some_and(|windows| !windows.parent.borrow().is_hidden())
}
pub fn show_in_game_chat(
    immediate: bool,
    manager: &mut WindowManager,
    effects: &mut WindowEventEffects<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    show_selected(immediate, manager, effects, None)
}
fn show_selected(
    _immediate: bool,
    manager: &mut WindowManager,
    effects: &mut WindowEventEffects<'_>,
    mut selected: Option<&mut GameWindow>,
) -> Result<(), Box<dyn std::error::Error>> {
    if should_block_chat(effects)? {
        return Ok(());
    }
    let _facts = chat_world(effects)?; // Reject missing native recipients before callbacks.
    let existing = windows(effects);
    let windows = ensure_layout(manager, effects)?;
    if existing.is_some() {
        manager.hide_window_with_selected(&windows.parent, false, selected.as_deref_mut())?;
        manager.enable_window_with_selected(&windows.parent, true, selected.as_deref_mut())?;
        manager.hide_window_with_selected(&windows.text_entry, false, selected.as_deref_mut())?;
        manager.enable_window_with_selected(&windows.text_entry, true, selected.as_deref_mut())?;
        let saved = effects.with_ingame_ui(|owner| owner.chat.ui.saved_text.clone());
        set_entry_text(&windows.text_entry, &saved, selected.as_deref_mut());
        effects.with_ingame_ui(|owner| owner.chat.ui.saved_text.clear());
    }
    manager.set_focus_with_selected(Some(&windows.text_entry), effects, selected.as_deref_mut())?;
    // Focus callbacks completed. Re-resolve the canonical owner at this point.
    set_type_selected(InGameChatType::Everyone, effects, selected.as_deref_mut())?;
    effects.with_ingame_ui(|owner| {
        owner.chat.active = matches!(owner.chat.ui.phase, ChatLayoutPhase::Live(_))
    });
    Ok(())
}
pub fn hide_in_game_chat(
    immediate: bool,
    manager: &mut WindowManager,
    effects: &mut WindowEventEffects<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    hide_selected(immediate, manager, effects, None)
}
fn hide_selected(
    _immediate: bool,
    manager: &mut WindowManager,
    effects: &mut WindowEventEffects<'_>,
    mut selected: Option<&mut GameWindow>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(windows) = windows(effects) {
        let text = entry_text(&windows.text_entry, selected.as_deref_mut());
        effects.with_ingame_ui(|owner| owner.chat.ui.saved_text = text);
        manager.hide_window_with_selected(&windows.parent, true, selected.as_deref_mut())?;
        manager.enable_window_with_selected(&windows.parent, false, selected.as_deref_mut())?;
        manager.hide_window_with_selected(&windows.text_entry, true, selected.as_deref_mut())?;
        manager.enable_window_with_selected(&windows.text_entry, false, selected.as_deref_mut())?;
        manager.set_focus_with_selected(None, effects, selected.as_deref_mut())?;
    }
    manager.set_focus_with_selected(None, effects, selected)?;
    effects.with_ingame_ui(|owner| owner.chat.active = false);
    Ok(())
}
pub fn reset_in_game_chat(
    manager: &mut WindowManager,
    effects: &mut WindowEventEffects<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    let retiring = effects.with_ingame_ui(|owner| {
        if matches!(owner.chat.ui.phase, ChatLayoutPhase::Retiring(_)) {
            return None;
        }
        let windows = owner.chat.ui.phase.windows().cloned();
        if let Some(windows) = &windows {
            owner.chat.ui.phase = ChatLayoutPhase::Retiring(windows.clone());
        }
        Some((owner.chat.ui.generation, windows))
    });
    let Some((generation, retiring)) = retiring else {
        return Ok(());
    };
    if let Some(retiring) = retiring {
        manager.destroy_layout(&retiring.layout, effects);
    }
    effects.with_ingame_ui(|owner| {
        if owner.chat.ui.generation == generation {
            owner.chat.ui.phase = ChatLayoutPhase::Absent;
            owner.chat.ui.generation = generation.wrapping_add(1);
            owner.chat.ui.saved_text.clear();
            owner.chat.active = false;
            owner.chat.history.clear();
        }
    });
    Ok(())
}
pub fn toggle_in_game_chat(
    immediate: bool,
    manager: &mut WindowManager,
    effects: &mut WindowEventEffects<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    toggle_selected(immediate, manager, effects, None)
}
fn toggle_selected(
    immediate: bool,
    manager: &mut WindowManager,
    effects: &mut WindowEventEffects<'_>,
    mut selected: Option<&mut GameWindow>,
) -> Result<(), Box<dyn std::error::Error>> {
    // C++ ToggleInGameChat: justHid is consumed before the replay/type gates.
    if effects.with_ingame_ui(|owner| {
        let skip = owner.chat.ui.just_hid;
        owner.chat.ui.just_hid = false;
        skip
    }) {
        return Ok(());
    }
    if should_block_chat(effects)? {
        return Ok(());
    }
    let facts = chat_world(effects)?;
    if !facts.multiplayer && get_global_data().is_some_and(|data| data.read().net_min_players > 0) {
        return Ok(());
    }
    let Some(windows) = windows(effects) else {
        return show_selected(immediate, manager, effects, selected);
    };
    if with_window(&windows.parent, selected.as_deref_mut(), |parent| {
        parent.is_hidden()
    }) {
        return show_selected(immediate, manager, effects, selected);
    }
    let text = entry_text(&windows.text_entry, selected.as_deref_mut())
        .trim()
        .to_owned();
    if !text.is_empty() && !handle_slash_commands(&text, effects) {
        let chat_type = get_in_game_chat_type(effects);
        let mask = facts.player_mask(&chat_type);
        let mut filtered = text;
        get_language_filter().filter_line(&mut filtered);
        if let Some(network) = game_network::get_network() {
            let _ = pollster::block_on(network.send_chat_message(filtered.clone(), mask as u8));
        } else {
            warn!("send_chat ignored; network not initialized");
        }
        if let Some(sender) = facts.local_slot {
            push_network_chat_message(sender, filtered, mask, false, effects);
        }
    }
    set_entry_text(&windows.text_entry, "", selected.as_deref_mut());
    hide_selected(immediate, manager, effects, selected)?;
    effects.with_ingame_ui(|owner| owner.chat.ui.just_hid = true);
    Ok(())
}
pub fn push_network_chat_message(
    sender_id: u8,
    message: String,
    target_mask: i32,
    is_disconnect: bool,
    effects: &mut WindowEventEffects<'_>,
) {
    let line = effects.with_ingame_ui(|owner| {
        owner
            .chat
            .receive_network_message(sender_id, message, target_mask, is_disconnect)
    });
    effects.message(&line);
}
pub fn in_game_chat_system_receiving(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut WindowEventEffects<'_>,
) -> WindowMsgHandled {
    if msg == WindowMessage::InputFocus {
        return write_input_focus_response(data1, data2, true);
    }
    if !live_callback(window, effects) {
        return WindowMsgHandled::Ignored;
    }
    match msg {
        WindowMessage::GadgetEditDone => {
            match effects.with_manager(|manager, effects| {
                toggle_selected(false, manager, effects, Some(window))
            }) {
                Ok(()) => WindowMsgHandled::Handled,
                Err(error) => {
                    log::error!("receiving chat toggle failed: {error}");
                    WindowMsgHandled::Ignored
                }
            }
        }
        WindowMessage::GadgetSelected => {
            if data1 as u32 == NameKeyGenerator::name_to_key("InGameChat.wnd:ButtonClear") {
                if let Some(windows) = windows(effects) {
                    set_entry_text(&windows.text_entry, "", Some(window));
                }
                effects.with_ingame_ui(|owner| owner.chat.ui.saved_text.clear());
            }
            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}
pub fn in_game_chat_input_receiving(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
    effects: &mut WindowEventEffects<'_>,
) -> WindowMsgHandled {
    if msg != WindowMessage::Char || !live_callback(window, effects) {
        return WindowMsgHandled::Ignored;
    }
    if data1 == KEY_ESC {
        if let Err(error) = effects
            .with_manager(|manager, effects| hide_selected(false, manager, effects, Some(window)))
        {
            log::error!("receiving chat hide failed: {error}");
            return WindowMsgHandled::Ignored;
        }
    }
    WindowMsgHandled::Handled
}

#[cfg(test)]
mod owner_tests;
