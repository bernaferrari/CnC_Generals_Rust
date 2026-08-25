//! PopupSaveLoad.cpp callback port.

use crate::game_text::GameText;
use crate::gui::callbacks::quit_menu::destroy_quit_menu;
use crate::gui::campaign_manager::get_campaign_manager;
use crate::gui::control_bar::{
    HostControlBarInputProvenance, host_control_bar_input_provenance_for_current_dispatch,
};
use crate::gui::gadgets::ListBoxItemData;
use crate::gui::menu_flags::{
    get_dont_show_main_menu, get_replay_was_pressed, set_replay_was_pressed,
};
use crate::gui::shell::Color as WindowColor;
use crate::gui::{
    GLM_DOUBLE_CLICKED, GameWindow, KeyModifiers, WindowLayout, WindowMessage, WindowMsgData,
    WindowMsgHandled, queue_set_focus, queue_shell_hide, queue_shell_pop, queue_shell_show,
    queue_shell_shutdown_complete, queue_window_manager_op, queue_window_manager_op_deferred,
    show_shell_map_if_available, with_shell_ref, with_window_manager, write_input_focus_response,
};
use game_engine::common::game_engine::get_game_engine;
use game_engine::common::ini::get_global_data;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::{
    AvailableGameInfo, SaveCode, SaveFileType, SaveLoadLayoutType, SnapshotType, get_game_state,
};
use gamelogic::helpers::TheGameLogic;
use gamelogic::system::game_logic::GAME_SINGLE_PLAYER;
use std::cell::RefCell;
use std::fs;
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};

const KEY_ESC: usize = 0x1B;
const KEY_STATE_UP: usize = 0x0001;
const DIFFICULTY_NORMAL: i32 = 1;

/// A save row supplied by the active runtime host.
///
/// The retail callback normally discovers rows from Common's `TheGameState`.
/// A host that owns a different snapshot implementation can publish its real
/// save inventory here without manufacturing WND controls or pretending that
/// Common can deserialize the host's format.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PopupSaveLoadEntry {
    pub filename: String,
    pub description: String,
    pub display_time: String,
    pub display_date: String,
    pub is_mission: bool,
}

/// A completed action from the retail PopupSaveLoad controls.
///
/// Requests are emitted only after `ButtonSaveDescConfirm`,
/// `ButtonOverwriteConfirm`, or `ButtonLoadConfirm`.  Merely pressing Save or
/// Load does not enqueue work: the real confirmation UI remains authoritative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PopupSaveLoadRequest {
    Save {
        filename: String,
        description: String,
        save_file_type: SaveFileType,
    },
    Load {
        filename: String,
    },
}

/// A host PopupSaveLoad request plus provenance captured at its origin.
///
/// Popup callbacks run in the same synchronous WND dispatch as the Control
/// Bar, so they share Main's safe, stack-scoped input context.  Only the three
/// explicit confirmation callbacks sample that context.  Other legitimate
/// Popup paths (such as the retail shell's immediate load) intentionally use
/// [`HostControlBarInputProvenance::InjectedOrUnknown`] so they cannot be
/// mistaken for a physical confirmation by the authoritative host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostPopupSaveLoadPublishedRequest {
    pub request: PopupSaveLoadRequest,
    pub input_provenance: HostControlBarInputProvenance,
}

#[derive(Default)]
struct HostPopupSaveLoadBridge {
    installed: bool,
    entries: Vec<PopupSaveLoadEntry>,
    requests: Vec<HostPopupSaveLoadPublishedRequest>,
}

static HOST_POPUP_SAVE_LOAD_BRIDGE: OnceLock<Mutex<HostPopupSaveLoadBridge>> = OnceLock::new();

fn host_popup_save_load_bridge() -> &'static Mutex<HostPopupSaveLoadBridge> {
    HOST_POPUP_SAVE_LOAD_BRIDGE.get_or_init(|| Mutex::new(HostPopupSaveLoadBridge::default()))
}

/// Publish the active host's save rows for the next PopupSaveLoad initialization.
///
/// Calling this also installs the host action bridge: later confirmed Save/Load
/// actions are queued for [`take_host_popup_save_load_published_requests`]
/// instead of being sent to Common's unrelated `TheGameState` snapshot
/// implementation.  [`take_host_popup_save_load_requests`] remains available
/// for standalone callers that need only the legacy request payload.
/// An empty inventory is still authoritative: it renders only the retail
/// `New Save Game` pseudo-row.  This prevents incompatible/stale Common saves
/// from leaking into a host-owned snapshot menu.
pub fn publish_host_popup_save_load_entries(entries: Vec<PopupSaveLoadEntry>) {
    if let Ok(mut bridge) = host_popup_save_load_bridge().lock() {
        bridge.installed = true;
        bridge.entries = entries;
    }
}

/// Drain confirmed PopupSaveLoad actions with their captured input provenance.
///
/// Main must use this detailed drain at its authority boundary.  The legacy
/// request-only drain below intentionally discards provenance.
pub fn take_host_popup_save_load_published_requests() -> Vec<HostPopupSaveLoadPublishedRequest> {
    host_popup_save_load_bridge()
        .lock()
        .map(|mut bridge| std::mem::take(&mut bridge.requests))
        .unwrap_or_default()
}

/// Drain confirmed PopupSaveLoad actions for legacy standalone callers.
pub fn take_host_popup_save_load_requests() -> Vec<PopupSaveLoadRequest> {
    take_host_popup_save_load_published_requests()
        .into_iter()
        .map(|published| published.request)
        .collect()
}

/// Disable the host bridge and discard its published rows/undrained requests.
/// Primarily useful when a host shuts down or tests reset the singleton state.
pub fn clear_host_popup_save_load_bridge() {
    if let Ok(mut bridge) = host_popup_save_load_bridge().lock() {
        *bridge = HostPopupSaveLoadBridge::default();
    }
}

fn host_popup_save_load_entries() -> Vec<PopupSaveLoadEntry> {
    host_popup_save_load_bridge()
        .lock()
        .map(|bridge| bridge.entries.clone())
        .unwrap_or_default()
}

fn host_popup_save_load_bridge_installed() -> bool {
    host_popup_save_load_bridge()
        .lock()
        .map(|bridge| bridge.installed)
        .unwrap_or(false)
}

fn queue_host_popup_save_load_request(
    request: PopupSaveLoadRequest,
    input_provenance: HostControlBarInputProvenance,
) -> bool {
    let Ok(mut bridge) = host_popup_save_load_bridge().lock() else {
        return false;
    };
    if !bridge.installed {
        return false;
    }
    bridge.requests.push(HostPopupSaveLoadPublishedRequest {
        request,
        input_provenance,
    });
    true
}

struct SaveLoadMenuState {
    button_back: i32,
    button_save: i32,
    button_load: i32,
    button_delete: i32,
    listbox_games: i32,
    button_overwrite_cancel: i32,
    button_overwrite_confirm: i32,
    button_load_cancel: i32,
    button_load_confirm: i32,
    button_save_desc_cancel: i32,
    button_save_desc_confirm: i32,
    button_delete_confirm: i32,
    button_delete_cancel: i32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    button_frame: Option<Rc<RefCell<GameWindow>>>,
    button_save_window: Option<Rc<RefCell<GameWindow>>>,
    button_load_window: Option<Rc<RefCell<GameWindow>>>,
    button_delete_window: Option<Rc<RefCell<GameWindow>>>,
    overwrite_confirm: Option<Rc<RefCell<GameWindow>>>,
    load_confirm: Option<Rc<RefCell<GameWindow>>>,
    save_desc: Option<Rc<RefCell<GameWindow>>>,
    listbox_games_window: Option<Rc<RefCell<GameWindow>>>,
    edit_desc: Option<Rc<RefCell<GameWindow>>>,
    delete_confirm: Option<Rc<RefCell<GameWindow>>>,
    /// Immutable host snapshot that populated the current listbox.  Keeping
    /// this with the visual rows means a confirmed request always carries the
    /// exact filename the player selected, even if the host refreshes its
    /// inventory while the dialog is open.
    host_entries: Vec<PopupSaveLoadEntry>,
    /// Whether the current rows belong to an external host.  This is distinct
    /// from `host_entries.is_empty()`: an installed host with no saves must not
    /// fall back to Common's incompatible save format.
    host_bridge_active: bool,
    current_layout_type: SaveLoadLayoutType,
    is_popup: bool,
    initial_gadget_delay: i32,
    just_entered: bool,
    is_shutting_down: bool,
}

impl Default for SaveLoadMenuState {
    fn default() -> Self {
        Self {
            button_back: 0,
            button_save: 0,
            button_load: 0,
            button_delete: 0,
            listbox_games: 0,
            button_overwrite_cancel: 0,
            button_overwrite_confirm: 0,
            button_load_cancel: 0,
            button_load_confirm: 0,
            button_save_desc_cancel: 0,
            button_save_desc_confirm: 0,
            button_delete_confirm: 0,
            button_delete_cancel: 0,
            parent: None,
            button_frame: None,
            button_save_window: None,
            button_load_window: None,
            button_delete_window: None,
            overwrite_confirm: None,
            load_confirm: None,
            save_desc: None,
            listbox_games_window: None,
            edit_desc: None,
            delete_confirm: None,
            host_entries: Vec::new(),
            host_bridge_active: false,
            current_layout_type: SaveLoadLayoutType::SaveAndLoad,
            is_popup: false,
            initial_gadget_delay: 0,
            just_entered: false,
            is_shutting_down: false,
        }
    }
}

thread_local! {
    static SAVE_LOAD_MENU_STATE: Arc<Mutex<SaveLoadMenuState>> =
        Arc::new(Mutex::new(SaveLoadMenuState::default()));
}

fn save_load_menu_state() -> Arc<Mutex<SaveLoadMenuState>> {
    SAVE_LOAD_MENU_STATE.with(|state| state.clone())
}

fn init_gadget_ids(state: &mut SaveLoadMenuState, prefix: &str) {
    state.button_back = NameKeyGenerator::name_to_key(&format!("{prefix}:ButtonBack")) as i32;
    state.button_save = NameKeyGenerator::name_to_key(&format!("{prefix}:ButtonSave")) as i32;
    state.button_load = NameKeyGenerator::name_to_key(&format!("{prefix}:ButtonLoad")) as i32;
    state.button_delete = NameKeyGenerator::name_to_key(&format!("{prefix}:ButtonDelete")) as i32;
    state.listbox_games = NameKeyGenerator::name_to_key(&format!("{prefix}:ListboxGames")) as i32;
    state.button_overwrite_cancel =
        NameKeyGenerator::name_to_key(&format!("{prefix}:ButtonOverwriteCancel")) as i32;
    state.button_overwrite_confirm =
        NameKeyGenerator::name_to_key(&format!("{prefix}:ButtonOverwriteConfirm")) as i32;
    state.button_load_cancel =
        NameKeyGenerator::name_to_key(&format!("{prefix}:ButtonLoadCancel")) as i32;
    state.button_load_confirm =
        NameKeyGenerator::name_to_key(&format!("{prefix}:ButtonLoadConfirm")) as i32;
    state.button_save_desc_cancel =
        NameKeyGenerator::name_to_key(&format!("{prefix}:ButtonSaveDescCancel")) as i32;
    state.button_save_desc_confirm =
        NameKeyGenerator::name_to_key(&format!("{prefix}:ButtonSaveDescConfirm")) as i32;
    state.button_delete_confirm =
        NameKeyGenerator::name_to_key(&format!("{prefix}:ButtonDeleteConfirm")) as i32;
    state.button_delete_cancel =
        NameKeyGenerator::name_to_key(&format!("{prefix}:ButtonDeleteCancel")) as i32;
}

/// Find a control inside the layout that is currently being initialized.
///
/// `SaveLoadMenuInit` may be invoked by a `GadgetSelected` callback while the
/// WindowManager is dispatching input. Looking controls up through the global
/// manager in that situation fail-closes on Rust's re-entry guard, even though
/// the retail WND parser has already created every window. The layout owns the
/// same windows, so use that local tree just as the C++ callback uses its
/// `WindowLayout`/parent relationship.
fn layout_window_by_id(layout: &WindowLayout, id: i32) -> Option<Rc<RefCell<GameWindow>>> {
    layout
        .windows()
        .iter()
        .find(|window| window.borrow().get_id() == id)
        .cloned()
}

fn load_controls(
    state: &mut SaveLoadMenuState,
    layout: &WindowLayout,
    parent_id: i32,
    prefix: &str,
) {
    let control = |suffix: &str| {
        layout_window_by_id(
            layout,
            NameKeyGenerator::name_to_key(&format!("{prefix}:{suffix}")) as i32,
        )
    };

    state.parent = layout_window_by_id(layout, parent_id);
    state.button_frame = control("MenuButtonFrame");
    state.button_save_window = control("ButtonSave");
    state.button_load_window = control("ButtonLoad");
    state.button_delete_window = control("ButtonDelete");
    state.overwrite_confirm = control("OverwriteConfirmParent");
    state.load_confirm = control("LoadConfirmParent");
    state.save_desc = control("SaveDescParent");
    state.delete_confirm = control("DeleteConfirmParent");
    state.edit_desc = control("EntryDesc");
    state.listbox_games_window = control("ListboxGames");
}

fn normalize_default_save_description_from_map_name(mut default_desc: String) -> String {
    if let Some(pos) = default_desc.rfind('\\') {
        default_desc = default_desc[pos + 1..].to_string();
    }

    let char_len = default_desc.chars().count();
    if char_len >= 4 && default_desc.chars().nth(char_len - 4) == Some('.') {
        for _ in 0..4 {
            let _ = default_desc.pop();
        }
    }

    default_desc
}

fn set_edit_description(edit_control: &Rc<RefCell<GameWindow>>) {
    let mut default_desc = String::new();
    let mut used_campaign = false;
    {
        let manager = get_campaign_manager();
        if let (Some(campaign), Some(mission_number)) = (
            manager.get_current_campaign(),
            manager.get_current_mission_number(),
        ) {
            let campaign_label = GameText::fetch(&campaign.campaign_name_label);
            let label = if campaign_label.is_empty() {
                campaign.campaign_name_label.clone()
            } else {
                campaign_label
            };
            default_desc = format!("{} {}", label, mission_number + 1);
            used_campaign = true;
        }
    }

    if !used_campaign {
        if let Some(data) = get_global_data() {
            let data = data.read();
            default_desc = data.map_name.clone();
        }
    }

    if default_desc.is_empty() {
        return;
    }

    default_desc = normalize_default_save_description_from_map_name(default_desc);

    if let Some(widget) = edit_control.borrow_mut().text_entry_mut() {
        widget.set_text(default_desc);
    }
}

fn populate_save_game_listbox(state: &mut SaveLoadMenuState) {
    state.host_bridge_active = host_popup_save_load_bridge_installed();
    state.host_entries = if state.host_bridge_active {
        host_popup_save_load_entries()
    } else {
        Vec::new()
    };
    let Some(listbox) = state.listbox_games_window.as_ref() else {
        return;
    };
    let Ok(mut listbox_guard) = listbox.try_borrow_mut() else {
        return;
    };
    let Some(list_box) = listbox_guard.list_box_mut() else {
        return;
    };

    list_box.clear();

    if state.current_layout_type != SaveLoadLayoutType::LoadOnly {
        let new_game_text = GameText::fetch("GUI:NewSaveGame");
        let new_game_color = WindowColor::new(200, 200, 255, 255);
        list_box.add_item_with_data_and_color(-1, &new_game_text, None, Some(new_game_color));
    }

    // An installed host owns the source even when the inventory is empty.
    // The standalone Common path remains the fallback only when no external
    // host has installed the bridge at all.
    if state.host_bridge_active {
        for (index, entry) in state.host_entries.iter().enumerate() {
            let display_label = if entry.description.is_empty() {
                entry.filename.clone()
            } else {
                entry.description.clone()
            };
            // C++ populateSaveGameListbox (GameState.cpp:1194-1206): mission
            // saves are green; others alternate white / periwinkle. Time/date
            // fill columns 1 and 2.
            let color = if entry.is_mission {
                WindowColor::new(200, 255, 200, 255)
            } else if (index & 0x1) != 0 {
                WindowColor::new(255, 255, 255, 255)
            } else {
                WindowColor::new(170, 170, 235, 255)
            };
            let item_index = list_box.add_item_with_data_and_color(
                index as i32,
                &display_label,
                Some(ListBoxItemData::Integer(index as i32)),
                Some(color),
            );
            let _ = list_box.set_item_column_data(
                item_index,
                1,
                ListBoxItemData::Text(entry.display_time.clone()),
            );
            let _ = list_box.set_item_column_color(item_index, 1, Some(color));
            let _ = list_box.set_item_column_data(
                item_index,
                2,
                ListBoxItemData::Text(entry.display_date.clone()),
            );
            let _ = list_box.set_item_column_color(item_index, 2, Some(color));
        }

        if !list_box.items().is_empty() {
            let _ = list_box.select_index(0, KeyModifiers::none());
        }
        return;
    }

    {
        let mut game_state = get_game_state();
        game_state.refresh_available_games();
    }

    let game_state = get_game_state();
    for (index, info) in game_state.available_games().iter().enumerate() {
        let mut display_label = info.save_game_info.description.clone();
        if display_label.is_empty() {
            let localized = GameText::fetch(&info.save_game_info.map_label);
            if localized.is_empty() || localized == info.save_game_info.map_label {
                display_label = info.save_game_info.map_label.clone();
            } else {
                display_label = localized;
            }
        }

        let date = &info.save_game_info.date;
        let display_time = format!("{:02}:{:02}", date.hour, date.minute);
        let display_date = format!("{:04}-{:02}-{:02}", date.year, date.month, date.day);

        let color = if info.save_game_info.save_file_type == SaveFileType::Mission {
            WindowColor::new(200, 255, 200, 255)
        } else if (index & 0x1) != 0 {
            WindowColor::new(255, 255, 255, 255)
        } else {
            WindowColor::new(170, 170, 235, 255)
        };

        let item_index = list_box.add_item_with_data_and_color(
            index as i32,
            &display_label,
            Some(ListBoxItemData::Integer(index as i32)),
            Some(color),
        );
        let _ = list_box.set_item_column_data(item_index, 1, ListBoxItemData::Text(display_time));
        let _ = list_box.set_item_column_color(item_index, 1, Some(color));
        let _ = list_box.set_item_column_data(item_index, 2, ListBoxItemData::Text(display_date));
        let _ = list_box.set_item_column_color(item_index, 2, Some(color));
    }

    if !list_box.items().is_empty() {
        let _ = list_box.select_index(0, KeyModifiers::none());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static HOST_POPUP_SAVE_LOAD_BRIDGE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn acquire_host_popup_save_load_bridge_test_guard() -> std::sync::MutexGuard<'static, ()> {
        HOST_POPUP_SAVE_LOAD_BRIDGE_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn default_save_description_uses_cpp_backslash_only_path_strip() {
        assert_eq!(
            normalize_default_save_description_from_map_name(
                "Maps\\USA\\Mission01.map".to_string()
            ),
            "Mission01"
        );
        assert_eq!(
            normalize_default_save_description_from_map_name("Maps/USA/Mission01.map".to_string()),
            "Maps/USA/Mission01"
        );
    }

    #[test]
    fn default_save_description_strips_any_cpp_four_char_extension() {
        assert_eq!(
            normalize_default_save_description_from_map_name("Skirmish.foo".to_string()),
            "Skirmish"
        );
        assert_eq!(
            normalize_default_save_description_from_map_name("Skirmish.long".to_string()),
            "Skirmish.long"
        );
    }

    #[test]
    fn save_load_menu_system_consumes_lifecycle_messages_like_cpp() {
        let window = GameWindow::new();

        assert_eq!(
            save_load_menu_system(&window, WindowMessage::Create, 0, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            save_load_menu_system(&window, WindowMessage::Destroy, 0, 0),
            WindowMsgHandled::Handled
        );
    }

    #[test]
    fn save_load_menu_system_handles_glm_double_clicked_like_cpp() {
        let listbox_id = 101;
        let listbox_window = Rc::new(RefCell::new(GameWindow::new()));
        let mut list_box = crate::gui::gadgets::ListBox::new(listbox_id as u32, 0, 0, 200, 80);
        list_box.add_item_with_data(0, "Existing save", Some(ListBoxItemData::Integer(0)));
        assert!(list_box.select_index(0, KeyModifiers::none()));
        listbox_window
            .borrow_mut()
            .set_widget(crate::gui::WindowWidget::ListBox(list_box));

        {
            let state_handle = save_load_menu_state();
            let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
            *state = SaveLoadMenuState::default();
            state.listbox_games = listbox_id;
            state.listbox_games_window = Some(listbox_window.clone());
        }

        let window = GameWindow::new();
        assert_eq!(
            save_load_menu_system(
                &window,
                WindowMessage::User(GLM_DOUBLE_CLICKED),
                listbox_id as WindowMsgData,
                (-1isize) as WindowMsgData,
            ),
            WindowMsgHandled::Handled
        );

        let selected = listbox_window
            .borrow_mut()
            .list_box_mut()
            .map(|list_box| list_box.selected_indices().to_vec())
            .unwrap_or_default();
        assert!(
            selected.is_empty(),
            "C++ GadgetListBoxSetSelected(-1) clears stale selection"
        );
    }

    #[test]
    fn first_real_save_row_never_uses_new_save_pseudo_row() {
        let mut listbox = crate::gui::gadgets::ListBox::new(1, 0, 0, 200, 80);
        listbox.add_item_with_data(-1, "New Save Game", Some(ListBoxItemData::Integer(-1)));
        listbox.add_item_with_data(0, "Existing save", Some(ListBoxItemData::Integer(0)));

        assert_eq!(first_real_save_row(&listbox), Some(1));

        listbox.clear();
        listbox.add_item_with_data(-1, "New Save Game", Some(ListBoxItemData::Integer(-1)));
        assert_eq!(first_real_save_row(&listbox), None);
    }

    #[test]
    fn host_bridge_emits_only_confirmed_typed_requests() {
        let _bridge_guard = acquire_host_popup_save_load_bridge_test_guard();
        clear_host_popup_save_load_bridge();
        publish_host_popup_save_load_entries(vec![PopupSaveLoadEntry {
            filename: "wnd_pause".into(),
            description: "Windowed pause save".into(),
            ..Default::default()
        }]);

        assert!(host_popup_save_load_bridge_installed());
        assert_eq!(host_popup_save_load_entries().len(), 1);
        assert!(take_host_popup_save_load_requests().is_empty());

        dispatch_save_from_popup_confirmation(
            "wnd_pause".into(),
            "Windowed pause save".into(),
            SaveFileType::Normal,
        );
        dispatch_load_from_popup_confirmation(SaveLoadSelection::Host(PopupSaveLoadEntry {
            filename: "wnd_pause".into(),
            description: "Windowed pause save".into(),
            ..Default::default()
        }));

        assert_eq!(
            take_host_popup_save_load_requests(),
            vec![
                PopupSaveLoadRequest::Save {
                    filename: "wnd_pause".into(),
                    description: "Windowed pause save".into(),
                    save_file_type: SaveFileType::Normal,
                },
                PopupSaveLoadRequest::Load {
                    filename: "wnd_pause".into(),
                },
            ]
        );
        clear_host_popup_save_load_bridge();
    }

    #[test]
    fn installed_empty_host_inventory_never_falls_back_to_common_rows() {
        let _bridge_guard = acquire_host_popup_save_load_bridge_test_guard();
        clear_host_popup_save_load_bridge();
        publish_host_popup_save_load_entries(Vec::new());

        let listbox_window = Rc::new(RefCell::new(GameWindow::new()));
        listbox_window
            .borrow_mut()
            .set_widget(crate::gui::WindowWidget::ListBox(
                crate::gui::gadgets::ListBox::new(1, 0, 0, 200, 80),
            ));
        let mut state = SaveLoadMenuState {
            current_layout_type: SaveLoadLayoutType::SaveAndLoad,
            listbox_games_window: Some(listbox_window.clone()),
            ..SaveLoadMenuState::default()
        };

        populate_save_game_listbox(&mut state);
        assert!(state.host_bridge_active);
        assert!(state.host_entries.is_empty());
        let mut listbox_window = listbox_window.borrow_mut();
        let listbox = listbox_window.list_box_mut().expect("test listbox");
        assert_eq!(listbox.items().len(), 1);
        assert_eq!(
            listbox.get_item_data(0),
            None,
            "the New Save Game pseudo-row intentionally has no selectable save data"
        );

        clear_host_popup_save_load_bridge();
    }

    #[test]
    fn named_live_load_confirms_the_exact_host_row() {
        let _bridge_guard = acquire_host_popup_save_load_bridge_test_guard();
        clear_host_popup_save_load_bridge();
        with_window_manager(|manager| manager.reset());
        // QuitMenu opens this popup while a game is running.  The retail
        // in-game path presents LoadConfirm; the shell path instead loads
        // directly, so model the former here.
        let previous_shell_active = crate::gui::shell::get_shell().is_shell_active();
        crate::gui::shell::get_shell().set_shell_active(false);
        publish_host_popup_save_load_entries(vec![
            PopupSaveLoadEntry {
                filename: "older_user_save".into(),
                description: "Older user save".into(),
                ..Default::default()
            },
            PopupSaveLoadEntry {
                filename: "wnd_pause".into(),
                description: "Windowed pause save".into(),
                ..Default::default()
            },
        ]);

        assert!(
            drive_os_wnd_popup_save_load_load_named_and_confirm_like_cpp("wnd_pause"),
            "the real listbox/load/confirmation widgets must accept the requested row"
        );
        assert_eq!(
            take_host_popup_save_load_requests(),
            vec![PopupSaveLoadRequest::Load {
                filename: "wnd_pause".into(),
            }],
            "a named load must never fall through to the first pre-existing row"
        );

        clear_host_popup_save_load_bridge();
        with_window_manager(|manager| manager.reset());
        crate::gui::shell::get_shell().set_shell_active(previous_shell_active);
    }

    #[test]
    fn live_save_description_confirm_emits_the_host_request() {
        let _bridge_guard = acquire_host_popup_save_load_bridge_test_guard();
        clear_host_popup_save_load_bridge();
        with_window_manager(|manager| manager.reset());
        let previous_shell_active = crate::gui::shell::get_shell().is_shell_active();
        crate::gui::shell::get_shell().set_shell_active(false);
        // The empty published inventory is intentional: it proves the real
        // New Save Game pseudo-row opens the description confirmation rather
        // than falling back to incompatible Common rows.
        publish_host_popup_save_load_entries(Vec::new());

        assert!(prepare_live_popup_save_load_for_click());
        assert!(crate::gui::dispatch_os_click_named_window(
            "PopupSaveLoad.wnd:ButtonSave"
        ));
        assert!(live_popup_save_load_window_visible(
            "PopupSaveLoad.wnd:SaveDescParent"
        ));

        let edit_desc = {
            let state_handle = save_load_menu_state();
            let state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
            state.edit_desc.clone().expect("retail EntryDesc")
        };
        edit_desc
            .borrow_mut()
            .text_entry_mut()
            .expect("retail EntryDesc text widget")
            .set_text("Windowed pause save");

        assert!(crate::gui::dispatch_os_click_named_window(
            "PopupSaveLoad.wnd:ButtonSaveDescConfirm"
        ));
        assert_eq!(
            take_host_popup_save_load_requests(),
            vec![PopupSaveLoadRequest::Save {
                filename: String::new(),
                description: "Windowed pause save".into(),
                save_file_type: SaveFileType::Normal,
            }]
        );

        clear_host_popup_save_load_bridge();
        with_window_manager(|manager| manager.reset());
        crate::gui::shell::get_shell().set_shell_active(previous_shell_active);
    }

    #[test]
    fn popup_confirmation_provenance_is_snapshotted_and_other_paths_fail_closed() {
        let _bridge_guard = acquire_host_popup_save_load_bridge_test_guard();
        clear_host_popup_save_load_bridge();
        publish_host_popup_save_load_entries(vec![PopupSaveLoadEntry {
            filename: "wnd_pause".into(),
            description: "Windowed pause save".into(),
            ..Default::default()
        }]);

        // No synchronous WND scope: even an explicit confirmation is unknown.
        dispatch_save_from_popup_confirmation(
            "wnd_pause".into(),
            "Windowed pause save".into(),
            SaveFileType::Normal,
        );
        crate::gui::control_bar::with_host_control_bar_input_provenance(
            HostControlBarInputProvenance::InjectedOrUnknown,
            || {
                dispatch_load_from_popup_confirmation(SaveLoadSelection::Host(
                    PopupSaveLoadEntry {
                        filename: "wnd_pause".into(),
                        description: "Windowed pause save".into(),
                        ..Default::default()
                    },
                ));
            },
        );
        crate::gui::control_bar::with_host_control_bar_input_provenance(
            HostControlBarInputProvenance::PhysicalWindowMouseInput,
            || {
                // Shell/double-click behavior remains valid, but has no
                // explicit confirmation provenance even under a real WND scope.
                dispatch_load_without_popup_confirmation(SaveLoadSelection::Host(
                    PopupSaveLoadEntry {
                        filename: "wnd_pause".into(),
                        description: "Windowed pause save".into(),
                        ..Default::default()
                    },
                ));
                dispatch_save_from_popup_confirmation(
                    "wnd_pause".into(),
                    "Windowed pause save".into(),
                    SaveFileType::Normal,
                );
                dispatch_load_from_popup_confirmation(SaveLoadSelection::Host(
                    PopupSaveLoadEntry {
                        filename: "wnd_pause".into(),
                        description: "Windowed pause save".into(),
                        ..Default::default()
                    },
                ));
            },
        );

        let provenance: Vec<_> = take_host_popup_save_load_published_requests()
            .into_iter()
            .map(|published| published.input_provenance)
            .collect();
        assert_eq!(
            provenance,
            vec![
                HostControlBarInputProvenance::InjectedOrUnknown,
                HostControlBarInputProvenance::InjectedOrUnknown,
                HostControlBarInputProvenance::InjectedOrUnknown,
                HostControlBarInputProvenance::PhysicalWindowMouseInput,
                HostControlBarInputProvenance::PhysicalWindowMouseInput,
            ],
            "only explicit physical Popup confirmation callbacks may carry physical evidence"
        );

        clear_host_popup_save_load_bridge();
    }
}

#[derive(Clone)]
enum SaveLoadSelection {
    Common(AvailableGameInfo),
    Host(PopupSaveLoadEntry),
}

impl SaveLoadSelection {
    fn filename(&self) -> &str {
        match self {
            Self::Common(info) => &info.filename,
            Self::Host(entry) => &entry.filename,
        }
    }

    fn description(&self) -> &str {
        match self {
            Self::Common(info) => &info.save_game_info.description,
            Self::Host(entry) => &entry.description,
        }
    }

    fn into_common(self) -> Option<AvailableGameInfo> {
        match self {
            Self::Common(info) => Some(info),
            Self::Host(_) => None,
        }
    }
}

fn selected_save(state: &SaveLoadMenuState) -> Option<SaveLoadSelection> {
    let listbox = state.listbox_games_window.as_ref()?;
    // A list-box GadgetSelected notification is forwarded while that list box
    // is still mutably borrowed by input dispatch.  Do not turn one delayed
    // redraw into a RefCell panic; the next queued menu-action update will
    // observe the selected row once dispatch has unwound.
    let mut listbox_guard = listbox.try_borrow_mut().ok()?;
    let list_box = listbox_guard.list_box_mut()?;
    let selected = list_box.selected_indices().first().copied()?;
    let data = list_box.get_item_data(selected)?;
    let index = match data {
        ListBoxItemData::Integer(value) if *value >= 0 => *value as usize,
        _ => return None,
    };

    if state.host_bridge_active {
        return state
            .host_entries
            .get(index)
            .cloned()
            .map(SaveLoadSelection::Host);
    }

    let game_state = get_game_state();
    game_state
        .available_games()
        .get(index)
        .cloned()
        .map(SaveLoadSelection::Common)
}

/// Mutate a control after the originating parent/system callback has released
/// its `RefCell` borrow.  Confirmation buttons are forwarded through their
/// parent containers, so hiding that container directly inside the callback
/// would otherwise panic (and made the retail SaveDesc cancel path unusable).
fn enable_window_rc(window: &Rc<RefCell<GameWindow>>, enabled: bool) {
    if let Ok(mut window) = window.try_borrow_mut() {
        let _ = window.enable(enabled);
        return;
    }

    let window = window.clone();
    queue_window_manager_op(move |_manager| {
        if let Ok(mut window) = window.try_borrow_mut() {
            let _ = window.enable(enabled);
        } else {
            let window = window.clone();
            queue_window_manager_op_deferred(move |_manager| {
                if let Ok(mut window) = window.try_borrow_mut() {
                    let _ = window.enable(enabled);
                }
            });
        }
    });
}

/// Hide/show a SaveLoad control through `WindowManager::hide_window`, not
/// `GameWindow::hide` directly. The latter captures a raw `GameWindow` pointer
/// while its `RefCell` is borrowed and can repeatedly requeue manager cleanup
/// during layout initialization. The manager path owns the `Rc`, clears focus/
/// modal state correctly, and defers once if a forwarded confirmation parent is
/// still borrowed by input dispatch.
fn hide_save_load_window(window: &Rc<RefCell<GameWindow>>, hide: bool) {
    let window = window.clone();
    queue_window_manager_op(move |manager| {
        if let Ok(probe) = window.try_borrow_mut() {
            // `hide_window` takes this same RefCell mutably.  Drop the probe
            // explicitly so this is never a transient nested borrow.
            drop(probe);
            let _ = manager.hide_window(&window, hide);
        } else {
            let window = window.clone();
            queue_window_manager_op_deferred(move |manager| {
                if let Ok(probe) = window.try_borrow_mut() {
                    drop(probe);
                    let _ = manager.hide_window(&window, hide);
                }
            });
        }
    });
}

fn first_real_save_row(listbox: &crate::gui::gadgets::ListBox) -> Option<usize> {
    (0..listbox.items().len()).find(|&row| {
        matches!(
            listbox.get_item_data(row),
            Some(ListBoxItemData::Integer(index)) if *index >= 0
        )
    })
}

fn set_listbox_selection_from_cpp_row(list_box: &mut crate::gui::gadgets::ListBox, row: i32) {
    if row < 0 {
        list_box.set_selected_indices(&[]);
    } else {
        let _ = list_box.select_index(row as usize, KeyModifiers::none());
    }
}

fn update_menu_actions(state: &SaveLoadMenuState) {
    let can_save = state.current_layout_type != SaveLoadLayoutType::LoadOnly;
    if let Some(save_button) = state.button_save_window.as_ref() {
        enable_window_rc(save_button, can_save);
    }

    let selected = selected_save(state);
    let has_selection = selected.is_some();
    if let Some(load_button) = state.button_load_window.as_ref() {
        enable_window_rc(load_button, has_selection);
    }
    if let Some(delete_button) = state.button_delete_window.as_ref() {
        // Host rows can be loaded/saved through the typed host bridge, but
        // this callback has no host delete request. Do not make a live button
        // claim an operation we cannot safely perform on the host format.
        enable_window_rc(
            delete_button,
            matches!(selected, Some(SaveLoadSelection::Common(_))),
        );
    }
}

fn close_save_menu(window: &GameWindow, is_popup: bool) {
    if is_popup {
        if let Some(layout) = window.get_layout() {
            layout.borrow().hide(true);
        }
    } else {
        queue_shell_hide();
    }
}

/// Deferred listbox actions run after the originating window's input borrow
/// has unwound. At that point the parsed parent can safely supply its layout
/// for the same popup close operation as `close_save_menu`.
fn close_save_menu_after_dispatch(state: &SaveLoadMenuState) {
    if state.is_popup {
        if let Some(layout) = state
            .parent
            .as_ref()
            .and_then(|parent| parent.try_borrow().ok())
            .and_then(|parent| parent.get_layout())
        {
            layout.borrow().hide(true);
        }
    } else {
        queue_shell_hide();
    }
}

fn do_load_game(selected: AvailableGameInfo) {
    let shell_active = with_shell_ref(|shell| shell.is_shell_active()).unwrap_or(false);
    if !shell_active {
        destroy_quit_menu();
    } else {
        with_window_manager(|manager| {
            manager.transition_remove("MainMenuLoadReplayMenu", false);
            manager.transition_remove("MainMenuLoadReplayMenuBack", false);
        });
        TheGameLogic::prepare_new_game(GAME_SINGLE_PLAYER, DIFFICULTY_NORMAL, 0);
    }

    let load_result = {
        let mut game_state = get_game_state();
        game_state.load_game(selected)
    };
    if !matches!(load_result, Ok(SaveCode::Ok)) {
        if TheGameLogic::is_in_game() {
            let _ = TheGameLogic::clear_game_data();
        }
        if let Some(engine) = get_game_engine() {
            let mut engine = engine.lock();
            let _ = pollster::block_on(engine.reset());
        }
        queue_shell_show(true);
    }
}

fn save_file_type_for_layout(layout_type: SaveLoadLayoutType) -> SaveFileType {
    if layout_type == SaveLoadLayoutType::SaveAndLoad {
        SaveFileType::Normal
    } else {
        SaveFileType::Mission
    }
}

/// Dispatch a Save through the active host bridge when present.
///
/// Fallback writes C++ `GameState::xferSaveData` 17 named chunks
/// (`System::SaveGame::GameState::save_game`). Host pause-save now uses the
/// same container (`SaveFileManager::write_common_sav_chunks`).
fn dispatch_save_request(
    filename: String,
    description: String,
    save_file_type: SaveFileType,
    input_provenance: HostControlBarInputProvenance,
) {
    if queue_host_popup_save_load_request(
        PopupSaveLoadRequest::Save {
            filename: filename.clone(),
            description: description.clone(),
            save_file_type,
        },
        input_provenance,
    ) {
        return;
    }

    let mut game_state = get_game_state();
    let _ = game_state.save_game(
        filename,
        description,
        save_file_type,
        SnapshotType::SaveLoad,
    );
}

/// Dispatch a Load through the active host bridge when present.
/// Common `TheGameState` is only used when no host bridge is installed.
fn dispatch_load_request(
    selected: SaveLoadSelection,
    input_provenance: HostControlBarInputProvenance,
) {
    if queue_host_popup_save_load_request(
        PopupSaveLoadRequest::Load {
            filename: selected.filename().to_string(),
        },
        input_provenance,
    ) {
        return;
    }

    if let Some(common) = selected.into_common() {
        do_load_game(common);
    }
}

/// Dispatch work from `ButtonOverwriteConfirm` or `ButtonSaveDescConfirm`.
///
/// Sampling provenance here, rather than at Main's delayed bridge tick,
/// preserves whether this exact confirmation originated in a physical WND
/// mouse event.  An unscoped caller fails closed as injected/unknown.
fn dispatch_save_from_popup_confirmation(
    filename: String,
    description: String,
    save_file_type: SaveFileType,
) {
    dispatch_save_request(
        filename,
        description,
        save_file_type,
        host_control_bar_input_provenance_for_current_dispatch(),
    );
}

/// Dispatch work from `ButtonLoadConfirm`, retaining its exact WND provenance.
fn dispatch_load_from_popup_confirmation(selected: SaveLoadSelection) {
    dispatch_load_request(
        selected,
        host_control_bar_input_provenance_for_current_dispatch(),
    );
}

/// Dispatch a legitimate Popup load path which did not pass through
/// `ButtonLoadConfirm` (the retail shell's immediate load or list double-click).
/// It must retain normal load behavior, but cannot serve as confirmation proof.
fn dispatch_load_without_popup_confirmation(selected: SaveLoadSelection) {
    dispatch_load_request(selected, HostControlBarInputProvenance::InjectedOrUnknown);
}

/// Apply the C++ Load-button UI transition.  A shell load completes immediately
/// in the original game; callers must dispatch the returned selection only
/// after releasing the menu mutex.
fn process_load_button_press(
    state: &mut SaveLoadMenuState,
    window: &GameWindow,
) -> Option<SaveLoadSelection> {
    let selected = selected_save(state)?;

    if with_shell_ref(|shell| shell.is_shell_active()).unwrap_or(false) {
        close_save_menu(window, state.is_popup);
        return Some(selected);
    }

    if let Some(listbox) = state.listbox_games_window.as_ref() {
        enable_window_rc(listbox, false);
    }
    if let Some(frame) = state.button_frame.as_ref() {
        enable_window_rc(frame, false);
    }
    if let Some(confirm) = state.load_confirm.as_ref() {
        hide_save_load_window(confirm, false);
    }
    None
}

/// Finish a listbox double-click after `GameWindow` has released its mutable
/// listbox borrow. The normal ButtonLoad path can run synchronously; a listbox
/// event cannot, because querying its selected row during dispatch would be a
/// `RefCell` re-entry.
fn process_load_double_click_after_dispatch(
    state: &mut SaveLoadMenuState,
    row_selected: i32,
) -> Option<SaveLoadSelection> {
    let listbox = state.listbox_games_window.as_ref()?;
    let mut listbox = listbox.try_borrow_mut().ok()?;
    let listbox_widget = listbox.list_box_mut()?;
    set_listbox_selection_from_cpp_row(listbox_widget, row_selected);
    drop(listbox);

    let selected = selected_save(state)?;
    if with_shell_ref(|shell| shell.is_shell_active()).unwrap_or(false) {
        close_save_menu_after_dispatch(state);
        return Some(selected);
    }

    if let Some(listbox) = state.listbox_games_window.as_ref() {
        enable_window_rc(listbox, false);
    }
    if let Some(frame) = state.button_frame.as_ref() {
        enable_window_rc(frame, false);
    }
    if let Some(confirm) = state.load_confirm.as_ref() {
        hide_save_load_window(confirm, false);
    }
    None
}

fn queue_menu_actions_refresh_after_dispatch() {
    let state_handle = save_load_menu_state();
    queue_window_manager_op(move |_manager| {
        let state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
        update_menu_actions(&state);
    });
}

fn selected_filename_and_description(selected: Option<SaveLoadSelection>) -> (String, String) {
    selected
        .as_ref()
        .map(|selected| {
            (
                selected.filename().to_string(),
                selected.description().to_string(),
            )
        })
        .unwrap_or_default()
}

fn selected_common_game_info(selected: Option<SaveLoadSelection>) -> Option<AvailableGameInfo> {
    selected.and_then(SaveLoadSelection::into_common)
}

fn should_show_delete_for_selection(selected: &Option<SaveLoadSelection>) -> bool {
    // Host snapshot ownership is deliberately write-only here: deleting a
    // host row through Common's save directory would target the wrong format.
    matches!(selected, Some(SaveLoadSelection::Common(_)))
}

fn process_delete_confirmed(state: &mut SaveLoadMenuState) {
    let selected = selected_common_game_info(selected_save(state));
    if let Some(selected) = selected {
        let game_state = get_game_state();
        let filepath = game_state.get_file_path_in_save_directory(&selected.filename);
        let _ = fs::remove_file(filepath);
    }
    populate_save_game_listbox(state);
}

pub fn save_load_menu_init(layout: &WindowLayout, user_data: Option<&dyn std::any::Any>) {
    let state_handle = save_load_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    state.current_layout_type = SaveLoadLayoutType::SaveAndLoad;
    state.is_popup = true;
    if let Some(layout_type) = user_data.and_then(|data| data.downcast_ref::<SaveLoadLayoutType>())
    {
        state.current_layout_type = *layout_type;
    }

    init_gadget_ids(&mut state, "PopupSaveLoad.wnd");
    let parent_id = NameKeyGenerator::name_to_key("PopupSaveLoad.wnd:SaveLoadMenu") as i32;
    load_controls(&mut state, layout, parent_id, "PopupSaveLoad.wnd");

    if let Some(frame) = state.button_frame.as_ref() {
        enable_window_rc(frame, true);
    }
    if let Some(window) = state.overwrite_confirm.as_ref() {
        hide_save_load_window(window, true);
    }
    if let Some(window) = state.load_confirm.as_ref() {
        hide_save_load_window(window, true);
    }
    if let Some(window) = state.save_desc.as_ref() {
        hide_save_load_window(window, true);
    }
    if let Some(window) = state.delete_confirm.as_ref() {
        hide_save_load_window(window, true);
    }

    populate_save_game_listbox(&mut state);
    update_menu_actions(&state);

    // Do not call back into WindowManager while this callback may itself be
    // running under WindowManager input dispatch. Queue the focus/modal work
    // after releasing the menu-state lock; the retail parent is already in
    // the parsed WND tree.
    let parent = state.parent.clone();
    drop(state);
    if let Some(parent) = parent {
        queue_set_focus(parent.clone());
        queue_window_manager_op(move |manager| {
            let _ = manager.set_modal(parent);
        });
    }
}

pub fn save_load_menu_full_screen_init(
    layout: &WindowLayout,
    user_data: Option<&dyn std::any::Any>,
) {
    show_shell_map_if_available(true);

    let state_handle = save_load_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    state.is_popup = false;
    state.current_layout_type = SaveLoadLayoutType::LoadOnly;
    if let Some(layout_type) = user_data.and_then(|data| data.downcast_ref::<SaveLoadLayoutType>())
    {
        state.current_layout_type = *layout_type;
    }

    init_gadget_ids(&mut state, "SaveLoad.wnd");
    let parent_id = NameKeyGenerator::name_to_key("SaveLoad.wnd:SaveLoadMenu") as i32;
    load_controls(&mut state, layout, parent_id, "SaveLoad.wnd");

    if let Some(frame) = state.button_frame.as_ref() {
        enable_window_rc(frame, true);
    }
    if let Some(window) = state.overwrite_confirm.as_ref() {
        hide_save_load_window(window, true);
    }
    if let Some(window) = state.load_confirm.as_ref() {
        hide_save_load_window(window, true);
    }
    if let Some(window) = state.save_desc.as_ref() {
        hide_save_load_window(window, true);
    }
    if let Some(window) = state.delete_confirm.as_ref() {
        hide_save_load_window(window, true);
    }

    populate_save_game_listbox(&mut state);
    update_menu_actions(&state);

    layout.hide(false);
    state.just_entered = true;
    state.initial_gadget_delay = 2;
    if let Some(parent) = state.parent.as_ref() {
        hide_save_load_window(parent, true);
    }
    state.is_shutting_down = false;

    let parent = state.parent.clone();
    drop(state);
    if let Some(parent) = parent {
        queue_set_focus(parent);
    }
}

pub fn save_load_menu_shutdown(layout: &WindowLayout, user_data: Option<&dyn std::any::Any>) {
    let pop_immediate = user_data
        .and_then(|data| data.downcast_ref::<bool>())
        .copied()
        .unwrap_or(false);

    if pop_immediate {
        layout.hide(true);
        queue_shell_shutdown_complete(false);
        return;
    }

    with_window_manager(|manager| {
        manager.transition_reverse("SaveLoadMenuFade");
    });
    let state_handle = save_load_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    state.is_shutting_down = true;
}

pub fn save_load_menu_update(layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    let state_handle = save_load_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    if get_dont_show_main_menu() && state.just_entered {
        state.just_entered = false;
    }
    if get_replay_was_pressed() && state.just_entered {
        state.just_entered = false;
        set_replay_was_pressed(false);
    }

    if state.just_entered {
        if state.initial_gadget_delay == 1 {
            with_window_manager(|manager| {
                manager.transition_remove("MainMenuDefaultMenuLogoFade", false);
                manager.transition_set_group("SaveLoadMenuFade", false);
            });
            state.initial_gadget_delay = 2;
            state.just_entered = false;
        } else {
            state.initial_gadget_delay -= 1;
        }
    }

    if state.is_shutting_down {
        let shell_finished = with_shell_ref(|shell| shell.is_anim_finished()).unwrap_or(false);
        let transitions_finished = with_window_manager(|manager| manager.transitions_finished());
        if shell_finished && transitions_finished {
            layout.hide(true);
            queue_shell_shutdown_complete(false);
        }
    }
}

pub fn save_load_menu_input(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    if msg != WindowMessage::Char || data1 != KEY_ESC {
        return WindowMsgHandled::Ignored;
    }
    if (data2 & KEY_STATE_UP) == 0 {
        return WindowMsgHandled::Handled;
    }

    let state_handle = save_load_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(confirm) = state.delete_confirm.as_ref() {
        hide_save_load_window(confirm, true);
    }
    if let Some(listbox) = state.listbox_games_window.as_ref() {
        enable_window_rc(listbox, true);
    }
    if let Some(frame) = state.button_frame.as_ref() {
        enable_window_rc(frame, true);
    }

    let parent = state.parent.clone();
    let button_back = state.button_back;
    drop(state);
    if let Some(parent) = parent {
        queue_window_manager_op(move |_manager| {
            if let Ok(mut parent) = parent.try_borrow_mut() {
                let _ = parent.send_system_message(
                    WindowMessage::GadgetSelected,
                    button_back as WindowMsgData,
                    0,
                );
            } else {
                let parent = parent.clone();
                queue_window_manager_op_deferred(move |_manager| {
                    if let Ok(mut parent) = parent.try_borrow_mut() {
                        let _ = parent.send_system_message(
                            WindowMessage::GadgetSelected,
                            button_back as WindowMsgData,
                            0,
                        );
                    }
                });
            }
        });
    }

    WindowMsgHandled::Handled
}

pub fn save_load_menu_system(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    let state_handle = save_load_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    match msg {
        WindowMessage::Create | WindowMessage::Destroy => WindowMsgHandled::Handled,
        WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
        WindowMessage::User(code) if code == GLM_DOUBLE_CLICKED => {
            if data1 as i32 == state.listbox_games {
                let row_selected = data2 as i32;
                drop(state);
                queue_window_manager_op(move |_manager| {
                    let state_handle = save_load_menu_state();
                    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
                    let selected =
                        process_load_double_click_after_dispatch(&mut state, row_selected);
                    drop(state);
                    if let Some(selected) = selected {
                        dispatch_load_without_popup_confirmation(selected);
                    }
                });
                return WindowMsgHandled::Handled;
            }
            WindowMsgHandled::Ignored
        }
        WindowMessage::GadgetSelected => {
            let control_id = data1 as i32;
            if control_id == state.listbox_games {
                // The listbox itself is still mutably borrowed by input
                // dispatch. Refresh its sibling action buttons after the
                // borrow unwinds, rather than falsely treating the selection
                // as absent or panicking on a re-entrant RefCell borrow.
                drop(state);
                queue_menu_actions_refresh_after_dispatch();
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_load {
                if let Some(selected) = process_load_button_press(&mut state, window) {
                    drop(state);
                    dispatch_load_without_popup_confirmation(selected);
                }
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_save {
                let selected = selected_save(&state);
                if selected.is_none() {
                    if let Some(save_desc) = state.save_desc.as_ref() {
                        hide_save_load_window(save_desc, false);
                    }
                    if let Some(edit_desc) = state.edit_desc.as_ref() {
                        set_edit_description(edit_desc);
                        queue_set_focus(edit_desc.clone());
                    }
                    if let Some(listbox) = state.listbox_games_window.as_ref() {
                        enable_window_rc(listbox, false);
                    }
                } else {
                    if let Some(listbox) = state.listbox_games_window.as_ref() {
                        enable_window_rc(listbox, false);
                    }
                    if let Some(frame) = state.button_frame.as_ref() {
                        enable_window_rc(frame, false);
                    }
                    if let Some(confirm) = state.overwrite_confirm.as_ref() {
                        hide_save_load_window(confirm, false);
                    }
                }
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_delete {
                let selected = selected_save(&state);
                if should_show_delete_for_selection(&selected) {
                    if let Some(listbox) = state.listbox_games_window.as_ref() {
                        enable_window_rc(listbox, false);
                    }
                    if let Some(frame) = state.button_frame.as_ref() {
                        enable_window_rc(frame, false);
                    }
                    if let Some(confirm) = state.delete_confirm.as_ref() {
                        hide_save_load_window(confirm, false);
                    }
                }
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_back {
                if state.is_popup {
                    close_save_menu(window, true);
                } else {
                    queue_shell_pop();
                }
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_delete_confirm || control_id == state.button_delete_cancel
            {
                if control_id == state.button_delete_confirm {
                    process_delete_confirmed(&mut state);
                }

                if let Some(confirm) = state.delete_confirm.as_ref() {
                    hide_save_load_window(confirm, true);
                }
                if let Some(listbox) = state.listbox_games_window.as_ref() {
                    enable_window_rc(listbox, true);
                }
                if let Some(frame) = state.button_frame.as_ref() {
                    enable_window_rc(frame, true);
                }
                update_menu_actions(&state);
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_overwrite_cancel
                || control_id == state.button_overwrite_confirm
            {
                if let Some(confirm) = state.overwrite_confirm.as_ref() {
                    hide_save_load_window(confirm, true);
                }

                if control_id == state.button_overwrite_confirm {
                    if let Some(listbox) = state.listbox_games_window.as_ref() {
                        enable_window_rc(listbox, true);
                    }
                    if let Some(frame) = state.button_frame.as_ref() {
                        enable_window_rc(frame, true);
                    }
                    update_menu_actions(&state);
                    close_save_menu(window, state.is_popup);

                    let file_type = save_file_type_for_layout(state.current_layout_type);
                    let (filename, desc) = selected_filename_and_description(selected_save(&state));
                    // Saving can serialize engine state and is not part of
                    // the UI-state critical section. Release it before the
                    // real save so nested UI/engine work cannot deadlock the
                    // menu mutex during a button dispatch.
                    drop(state);
                    dispatch_save_from_popup_confirmation(filename, desc, file_type);
                    return WindowMsgHandled::Handled;
                } else {
                    if let Some(frame) = state.button_frame.as_ref() {
                        enable_window_rc(frame, true);
                    }
                    update_menu_actions(&state);
                    if let Some(listbox) = state.listbox_games_window.as_ref() {
                        enable_window_rc(listbox, true);
                    }
                }

                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_save_desc_confirm {
                let desc = state
                    .edit_desc
                    .as_ref()
                    .and_then(|edit| {
                        let mut edit = edit.borrow_mut();
                        edit.text_entry_mut().map(|entry| entry.text().to_string())
                    })
                    .unwrap_or_default();

                if let Some(save_desc) = state.save_desc.as_ref() {
                    hide_save_load_window(save_desc, true);
                }
                if let Some(listbox) = state.listbox_games_window.as_ref() {
                    enable_window_rc(listbox, true);
                }
                if let Some(frame) = state.button_frame.as_ref() {
                    enable_window_rc(frame, true);
                }
                update_menu_actions(&state);
                close_save_menu(window, state.is_popup);

                let (filename, _) = selected_filename_and_description(selected_save(&state));
                let file_type = save_file_type_for_layout(state.current_layout_type);
                // See overwrite-confirm above: serialize only after dropping
                // the callback's menu-state lock.
                drop(state);
                dispatch_save_from_popup_confirmation(filename, desc, file_type);
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_save_desc_cancel {
                if let Some(save_desc) = state.save_desc.as_ref() {
                    hide_save_load_window(save_desc, true);
                }
                if let Some(listbox) = state.listbox_games_window.as_ref() {
                    enable_window_rc(listbox, true);
                }
                if let Some(frame) = state.button_frame.as_ref() {
                    enable_window_rc(frame, true);
                }
                update_menu_actions(&state);
                return WindowMsgHandled::Handled;
            }

            if control_id == state.button_load_confirm || control_id == state.button_load_cancel {
                if let Some(confirm) = state.load_confirm.as_ref() {
                    hide_save_load_window(confirm, true);
                }
                if let Some(listbox) = state.listbox_games_window.as_ref() {
                    enable_window_rc(listbox, true);
                }
                if let Some(frame) = state.button_frame.as_ref() {
                    enable_window_rc(frame, true);
                }
                update_menu_actions(&state);

                let selected = (control_id == state.button_load_confirm)
                    .then(|| selected_save(&state))
                    .flatten();
                if let Some(selected) = selected {
                    close_save_menu(window, state.is_popup);
                    // Loading destroys/rebuilds game state. It must not hold
                    // the UI menu mutex while it tears down QuitMenu/layouts.
                    drop(state);
                    dispatch_load_from_popup_confirmation(selected);
                }
                return WindowMsgHandled::Handled;
            }

            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}

/// Residual layout filename prefixes used by SaveLoad menus.
pub const SAVE_LOAD_LAYOUT_PREFIX_POPUP: &str = "PopupSaveLoad.wnd";
pub const SAVE_LOAD_LAYOUT_PREFIX_FULL: &str = "SaveLoad.wnd";

/// Residual: last SaveLoad action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualSaveLoadAction {
    None = 0,
    SelectSlot = 1,
    Load = 2,
    Save = 3,
    Delete = 4,
    Back = 5,
}

static RESIDUAL_SAVE_LOAD_ACTION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static RESIDUAL_SAVE_LOAD_SLOT: std::sync::atomic::AtomicI32 =
    std::sync::atomic::AtomicI32::new(-1);

fn residual_action_store(action: ResidualSaveLoadAction) {
    RESIDUAL_SAVE_LOAD_ACTION.store(action as u8, std::sync::atomic::Ordering::Relaxed);
}

/// Residual: last SaveLoad residual action.
pub fn residual_save_load_last_action() -> ResidualSaveLoadAction {
    match RESIDUAL_SAVE_LOAD_ACTION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ResidualSaveLoadAction::SelectSlot,
        2 => ResidualSaveLoadAction::Load,
        3 => ResidualSaveLoadAction::Save,
        4 => ResidualSaveLoadAction::Delete,
        5 => ResidualSaveLoadAction::Back,
        _ => ResidualSaveLoadAction::None,
    }
}

/// Residual: last selected slot index (-1 if none).
pub fn residual_save_load_selected_slot() -> Option<i32> {
    let slot = RESIDUAL_SAVE_LOAD_SLOT.load(std::sync::atomic::Ordering::Relaxed);
    if slot < 0 { None } else { Some(slot) }
}

/// Residual: bind SaveLoad gadget IDs for popup or full-screen layout.
pub fn simulate_save_load_menu_bind_layout(popup: bool, layout_type: SaveLoadLayoutType) -> bool {
    let prefix = if popup {
        SAVE_LOAD_LAYOUT_PREFIX_POPUP
    } else {
        SAVE_LOAD_LAYOUT_PREFIX_FULL
    };
    let state_handle = save_load_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    init_gadget_ids(&mut state, prefix);
    state.is_popup = popup;
    state.current_layout_type = layout_type;
    state.is_shutting_down = false;
    state.button_load != 0
        || state.button_save != 0
        || state.button_back != 0
        || state.listbox_games != 0
}

/// Residual: select a listbox games slot index (no live listbox widget required).
pub fn simulate_save_load_menu_select_slot(slot_index: i32) -> bool {
    if slot_index < 0 {
        return false;
    }
    let state_handle = save_load_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    if state.listbox_games == 0 {
        init_gadget_ids(&mut state, SAVE_LOAD_LAYOUT_PREFIX_FULL);
    }
    RESIDUAL_SAVE_LOAD_SLOT.store(slot_index, std::sync::atomic::Ordering::Relaxed);
    residual_action_store(ResidualSaveLoadAction::SelectSlot);
    true
}

/// Residual: fire ButtonLoad without full do_load_game (asset/engine reset).
/// Requires a prior select_slot residual. Latches Load action honesty.
pub fn simulate_save_load_menu_load_button_gadget_selected() -> bool {
    let state_handle = save_load_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    if state.button_load == 0 {
        init_gadget_ids(&mut state, SAVE_LOAD_LAYOUT_PREFIX_FULL);
    }
    if residual_save_load_selected_slot().is_none() {
        // C++ ignores Load with no selection.
        return false;
    }
    // Layout type LoadOnly / SaveAndLoad can load; SaveOnly cannot.
    if matches!(state.current_layout_type, SaveLoadLayoutType::SaveOnly) {
        return false;
    }
    residual_action_store(ResidualSaveLoadAction::Load);
    true
}

/// Residual: fire ButtonSave without full save file write.
pub fn simulate_save_load_menu_save_button_gadget_selected() -> bool {
    let state_handle = save_load_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    if state.button_save == 0 {
        init_gadget_ids(&mut state, SAVE_LOAD_LAYOUT_PREFIX_FULL);
    }
    if matches!(state.current_layout_type, SaveLoadLayoutType::LoadOnly) {
        return false;
    }
    residual_action_store(ResidualSaveLoadAction::Save);
    true
}

/// Residual: fire ButtonDelete without filesystem remove.
pub fn simulate_save_load_menu_delete_button_gadget_selected() -> bool {
    let state_handle = save_load_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    if state.button_delete == 0 {
        init_gadget_ids(&mut state, SAVE_LOAD_LAYOUT_PREFIX_FULL);
    }
    if residual_save_load_selected_slot().is_none() {
        return false;
    }
    residual_action_store(ResidualSaveLoadAction::Delete);
    true
}

/// Residual: fire ButtonBack (shell pop / popup hide residual latch).
pub fn simulate_save_load_menu_back_button_gadget_selected() -> bool {
    let state_handle = save_load_menu_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    if state.button_back == 0 {
        init_gadget_ids(&mut state, SAVE_LOAD_LAYOUT_PREFIX_FULL);
    }
    residual_action_store(ResidualSaveLoadAction::Back);
    RESIDUAL_SAVE_LOAD_SLOT.store(-1, std::sync::atomic::Ordering::Relaxed);
    true
}

/// Live WindowManager lookup for a `PopupSaveLoad.wnd:*` gadget.
///
/// Returns true only when the named window exists in the widget tree.
/// Residual `simulate_*` latches do **not** count.
pub fn live_popup_save_load_window_present(name: &str) -> bool {
    with_window_manager(|manager| manager.find_window_by_name(name).is_some())
}

const POPUP_SAVE_LOAD_PARENT: &str = "PopupSaveLoad.wnd:SaveLoadMenu";
const POPUP_SAVE_LOAD_RETAIL_CONTROLS: &[&str] = &[
    POPUP_SAVE_LOAD_PARENT,
    "PopupSaveLoad.wnd:MenuButtonFrame",
    "PopupSaveLoad.wnd:ButtonBack",
    "PopupSaveLoad.wnd:ButtonSave",
    "PopupSaveLoad.wnd:ButtonLoad",
    "PopupSaveLoad.wnd:ButtonDelete",
    "PopupSaveLoad.wnd:ListboxGames",
    "PopupSaveLoad.wnd:LoadConfirmParent",
    "PopupSaveLoad.wnd:ButtonLoadConfirm",
    "PopupSaveLoad.wnd:ButtonLoadCancel",
    "PopupSaveLoad.wnd:OverwriteConfirmParent",
    "PopupSaveLoad.wnd:ButtonOverwriteConfirm",
    "PopupSaveLoad.wnd:ButtonOverwriteCancel",
    "PopupSaveLoad.wnd:SaveDescParent",
    "PopupSaveLoad.wnd:ButtonSaveDescConfirm",
    "PopupSaveLoad.wnd:ButtonSaveDescCancel",
    "PopupSaveLoad.wnd:EntryDesc",
    "PopupSaveLoad.wnd:DeleteConfirmParent",
    "PopupSaveLoad.wnd:ButtonDeleteConfirm",
    "PopupSaveLoad.wnd:ButtonDeleteCancel",
];

fn live_popup_save_load_layout() -> Option<Rc<RefCell<WindowLayout>>> {
    with_window_manager(|manager| {
        manager
            .find_window_by_name(POPUP_SAVE_LOAD_PARENT)
            .and_then(|parent| parent.borrow().get_layout())
    })
}

fn retail_popup_save_load_controls_present() -> bool {
    with_window_manager(|manager| {
        POPUP_SAVE_LOAD_RETAIL_CONTROLS
            .iter()
            .all(|name| manager.find_window_by_name(name).is_some())
    })
}

fn popup_save_load_state_is_bound_to(layout: &Rc<RefCell<WindowLayout>>) -> bool {
    let Some(parent) = layout_window_by_id(
        &layout.borrow(),
        NameKeyGenerator::name_to_key(POPUP_SAVE_LOAD_PARENT) as i32,
    ) else {
        return false;
    };
    let state_handle = save_load_menu_state();
    let state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    state.is_popup
        && state
            .parent
            .as_ref()
            .is_some_and(|bound_parent| Rc::ptr_eq(bound_parent, &parent))
        && state.listbox_games_window.is_some()
}

fn hide_popup_save_load_confirmations(layout: &Rc<RefCell<WindowLayout>>) {
    let windows = layout.borrow().windows().to_vec();
    for window in windows {
        let is_confirmation = matches!(
            window.borrow().get_name(),
            "PopupSaveLoad.wnd:LoadConfirmParent"
                | "PopupSaveLoad.wnd:OverwriteConfirmParent"
                | "PopupSaveLoad.wnd:SaveDescParent"
                | "PopupSaveLoad.wnd:DeleteConfirmParent"
        );
        if is_confirmation {
            hide_save_load_window(&window, true);
        }
    }
}

/// Show a previously parsed popup while retaining the retail confirmation
/// windows' hidden state. `WindowLayout::hide(false)` restores every child, so
/// the confirmation parents must be hidden after that queued layout operation.
pub(crate) fn show_live_popup_save_load_layout(layout: Rc<RefCell<WindowLayout>>) {
    layout.borrow().hide(false);
    layout.borrow_mut().bring_forward();
    queue_window_manager_op(move |_manager| {
        hide_popup_save_load_confirmations(&layout);
    });
}

/// Create and initialize the retail `Menus/PopupSaveLoad.wnd` layout if it is
/// missing. There is deliberately no synthetic-widget fallback: the real WND
/// includes the listbox, confirmation dialogs, and callbacks a usable menu
/// requires.
pub fn ensure_live_popup_save_load_layout() -> bool {
    let layout = match live_popup_save_load_layout() {
        Some(layout) => layout,
        None => match with_window_manager(|manager| {
            manager
                .create_layout_with_windows("Menus/PopupSaveLoad.wnd")
                .ok()
                .map(|(layout, _)| layout)
        }) {
            Some(layout) => layout,
            None => return false,
        },
    };

    if !retail_popup_save_load_controls_present() {
        return false;
    }
    if !popup_save_load_state_is_bound_to(&layout) {
        layout.borrow().run_init(None);
    }

    retail_popup_save_load_controls_present() && popup_save_load_state_is_bound_to(&layout)
}

/// Show the fully parsed popup for an OS click. This preserves the C++ button
/// state: Load remains disabled when no real save-game row is selected.
pub fn prepare_live_popup_save_load_for_click() -> bool {
    if !ensure_live_popup_save_load_layout() {
        return false;
    }
    let Some(layout) = live_popup_save_load_layout() else {
        return false;
    };
    // Reopening a hidden popup mirrors QuitMenuSystem's `run_init` call and
    // restores focus/modal state after `close_save_menu` removed it. Avoid a
    // second modal push while an already initialized popup is visible.
    if layout.borrow().is_hidden() || !popup_save_load_state_is_bound_to(&layout) {
        layout.borrow().run_init(None);
    }
    show_live_popup_save_load_layout(layout);
    retail_popup_save_load_controls_present()
}

fn live_popup_save_load_window_visible(name: &str) -> bool {
    with_window_manager(|manager| {
        manager.find_window_by_name(name).is_some_and(|window| {
            let window = window.borrow();
            !window.is_hidden() && window.is_enabled()
        })
    })
}

/// Select a genuine save-game entry in the retail listbox and send the same
/// parent `GadgetSelected` notification that updates the C++ action buttons.
fn select_live_popup_save_game_matching_like_cpp(
    predicate: impl Fn(&SaveLoadSelection) -> bool,
) -> bool {
    if !prepare_live_popup_save_load_for_click() {
        return false;
    }

    let state_handle = save_load_menu_state();
    let state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    let Some(listbox_window) = state.listbox_games_window.as_ref().cloned() else {
        return false;
    };
    let Some(parent) = state.parent.as_ref().cloned() else {
        return false;
    };
    let listbox_id = state.listbox_games;

    let selected = {
        let Ok(mut listbox_window) = listbox_window.try_borrow_mut() else {
            return false;
        };
        let Some(listbox) = listbox_window.list_box_mut() else {
            return false;
        };
        (0..listbox.items().len())
            .find(|&row| {
                let Some(ListBoxItemData::Integer(index)) = listbox.get_item_data(row) else {
                    return false;
                };
                if *index < 0 {
                    return false;
                }
                let selection = if state.host_bridge_active {
                    state
                        .host_entries
                        .get(*index as usize)
                        .cloned()
                        .map(SaveLoadSelection::Host)
                } else {
                    let game_state = get_game_state();
                    game_state
                        .available_games()
                        .get(*index as usize)
                        .cloned()
                        .map(SaveLoadSelection::Common)
                };
                selection.as_ref().is_some_and(&predicate)
            })
            .is_some_and(|row| listbox.select_index(row, KeyModifiers::none()))
    };
    drop(state);
    if !selected {
        return false;
    }

    let handled = parent
        .borrow_mut()
        .send_system_message(
            WindowMessage::GadgetSelected,
            listbox_id as WindowMsgData,
            0,
        )
        .is_handled();
    handled
}

/// Select the first genuine save-game entry in the retail listbox.
///
/// The popup always places its `New Save Game` pseudo-row first, whose item
/// data is `-1`; it must never be used for a Load action.
pub fn select_first_live_popup_save_game_like_cpp() -> bool {
    select_live_popup_save_game_matching_like_cpp(|_| true)
}

/// Select the exact filename published in the current PopupSaveLoad list.
/// This is required for a host round-trip: loading the first row could select
/// an unrelated pre-existing user save.
pub fn select_live_popup_save_game_named_like_cpp(filename: &str) -> bool {
    if filename.is_empty() {
        return false;
    }
    select_live_popup_save_game_matching_like_cpp(|selection| selection.filename() == filename)
}

/// OS click on live `PopupSaveLoad.wnd:ButtonSave` (not `simulate_*`).
pub fn drive_os_wnd_popup_save_load_save_like_cpp() -> bool {
    let _ = prepare_live_popup_save_load_for_click();
    crate::gui::dispatch_os_click_named_window("PopupSaveLoad.wnd:ButtonSave")
}

/// Complete the retail save interaction: Save then the visible description or
/// overwrite confirmation. The actual `SaveLoadMenuSystem` callback performs
/// the GameState save; this helper only drives its live WND buttons.
pub fn drive_os_wnd_popup_save_load_save_and_confirm_like_cpp() -> bool {
    if !drive_os_wnd_popup_save_load_save_like_cpp() {
        return false;
    }
    if live_popup_save_load_window_visible("PopupSaveLoad.wnd:SaveDescParent") {
        return crate::gui::dispatch_os_click_named_window(
            "PopupSaveLoad.wnd:ButtonSaveDescConfirm",
        );
    }
    if live_popup_save_load_window_visible("PopupSaveLoad.wnd:OverwriteConfirmParent") {
        return crate::gui::dispatch_os_click_named_window(
            "PopupSaveLoad.wnd:ButtonOverwriteConfirm",
        );
    }
    false
}

/// OS click on live `PopupSaveLoad.wnd:ButtonLoad` (not `simulate_*`).
pub fn drive_os_wnd_popup_save_load_load_like_cpp() -> bool {
    let _ = prepare_live_popup_save_load_for_click();
    crate::gui::dispatch_os_click_named_window("PopupSaveLoad.wnd:ButtonLoad")
}

/// Complete the retail load interaction: re-open, select a real save row,
/// click Load, then accept LoadConfirm. It intentionally returns false when
/// no real save exists rather than enabling ButtonLoad or inventing a row.
pub fn drive_os_wnd_popup_save_load_load_and_confirm_like_cpp() -> bool {
    if !select_first_live_popup_save_game_like_cpp()
        || !crate::gui::dispatch_os_click_named_window("PopupSaveLoad.wnd:ButtonLoad")
    {
        return false;
    }
    if !live_popup_save_load_window_visible("PopupSaveLoad.wnd:LoadConfirmParent") {
        return false;
    }
    crate::gui::dispatch_os_click_named_window("PopupSaveLoad.wnd:ButtonLoadConfirm")
}

/// Complete the retail load interaction for one exact filename: re-open,
/// select that genuine row, click Load, and accept LoadConfirm.
pub fn drive_os_wnd_popup_save_load_load_named_and_confirm_like_cpp(filename: &str) -> bool {
    if !select_live_popup_save_game_named_like_cpp(filename)
        || !crate::gui::dispatch_os_click_named_window("PopupSaveLoad.wnd:ButtonLoad")
    {
        return false;
    }
    if !live_popup_save_load_window_visible("PopupSaveLoad.wnd:LoadConfirmParent") {
        return false;
    }
    crate::gui::dispatch_os_click_named_window("PopupSaveLoad.wnd:ButtonLoadConfirm")
}

/// Residual: full-screen LoadOnly path: bind + select slot + Load.
pub fn simulate_save_load_menu_prepare_load(slot_index: i32) -> bool {
    if !simulate_save_load_menu_bind_layout(false, SaveLoadLayoutType::LoadOnly) {
        return false;
    }
    if !simulate_save_load_menu_select_slot(slot_index) {
        return false;
    }
    simulate_save_load_menu_load_button_gadget_selected()
}
