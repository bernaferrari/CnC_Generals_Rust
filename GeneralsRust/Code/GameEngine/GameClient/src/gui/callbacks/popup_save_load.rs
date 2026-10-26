//! PopupSaveLoad.cpp callback port.

use crate::game_text::GameText;
use crate::gui::callbacks::quit_menu::destroy_quit_menu;

use crate::gui::control_bar::HostControlBarInputProvenance;
use crate::gui::gadgets::ListBoxItemData;
use crate::gui::menu_flags::{
    get_dont_show_main_menu, get_replay_was_pressed, set_replay_was_pressed,
};
use crate::gui::shell::Color as WindowColor;
use crate::gui::{
    GLM_DOUBLE_CLICKED, GameWindow, KeyModifiers, WindowLayout, WindowMessage, WindowMsgData,
    WindowMsgHandled, queue_shell_hide, queue_shell_pop, queue_shell_show,
    queue_shell_shutdown_complete, show_shell_map_if_available, write_input_focus_response,
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
pub struct HostPopupSaveLoadBridge {
    installed: bool,
    entries: Vec<PopupSaveLoadEntry>,
    requests: Vec<HostPopupSaveLoadPublishedRequest>,
}

impl HostPopupSaveLoadBridge {
    pub fn publish_entries(&mut self, entries: Vec<PopupSaveLoadEntry>) {
        self.installed = true;
        self.entries = entries;
    }
    pub fn is_installed(&self) -> bool {
        self.installed
    }
    pub fn take_requests(&mut self) -> Vec<HostPopupSaveLoadPublishedRequest> {
        std::mem::take(&mut self.requests)
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

pub fn publish_host_popup_save_load_entries(
    entries: Vec<PopupSaveLoadEntry>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    effects.save_load_host_mut().publish_entries(entries);
}
pub fn take_host_popup_save_load_published_requests(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> Vec<HostPopupSaveLoadPublishedRequest> {
    effects.save_load_host_mut().take_requests()
}
pub fn take_host_popup_save_load_requests(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> Vec<PopupSaveLoadRequest> {
    take_host_popup_save_load_published_requests(effects)
        .into_iter()
        .map(|published| published.request)
        .collect()
}
pub fn clear_host_popup_save_load_bridge(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    effects.save_load_host_mut().clear();
}
fn queue_host_popup_save_load_request(
    request: PopupSaveLoadRequest,
    input_provenance: HostControlBarInputProvenance,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    // Native owns the transport even before it has published its first (possibly empty) inventory.
    let native =
        effects.input_authority() == crate::core::game_client::ClientInputAuthority::NativeHosted;
    let bridge = effects.save_load_host_mut();
    if !bridge.installed && !native {
        return false;
    }
    bridge.requests.push(HostPopupSaveLoadPublishedRequest {
        request,
        input_provenance,
    });
    true
}

pub struct SaveLoadMenuState {
    binding_generation: u64,
    residual_action: u8,
    residual_slot: Option<i32>,
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
            binding_generation: 0,
            residual_action: 0,
            residual_slot: None,
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

    state.binding_generation = state.binding_generation.wrapping_add(1);
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

fn set_edit_description(
    edit_control: &Rc<RefCell<GameWindow>>,
    campaign_manager: Option<&crate::gui::campaign_manager::CampaignManager>,
) {
    let mut default_desc = String::new();
    let mut used_campaign = false;
    {
        if let Some(manager) = campaign_manager {
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

fn populate_receiving_save_game_listbox(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    let native =
        effects.input_authority() == crate::core::game_client::ClientInputAuthority::NativeHosted;
    let bridge = effects.save_load_host();
    let entries = (native || bridge.installed).then(|| bridge.entries.clone());
    populate_save_game_listbox(effects.save_load_mut(), entries);
}

fn populate_save_game_listbox(
    state: &mut SaveLoadMenuState,
    host_entries: Option<Vec<PopupSaveLoadEntry>>,
) {
    state.host_bridge_active = host_entries.is_some();
    state.host_entries = host_entries.unwrap_or_default();
    let Some(listbox) = state.listbox_games_window.as_ref() else {
        return;
    };
    let mut listbox_guard = listbox.borrow_mut();
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

    fn with_test_effects(f: impl FnOnce(&mut crate::gui::game_window::WindowEventEffects<'_>)) {
        let mut client = crate::core::GameClient::new_with_input_authority(
            crate::core::game_client::ClientInputAuthority::NativeHosted,
        )
        .unwrap();
        client.set_in_game_ui_subsystem(crate::core::subsystems::InGameUISubsystem::default());
        client.with_window_event_effects(None, f);
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
        with_test_effects(|effects| {
            let mut window = GameWindow::new();

            assert_eq!(
                save_load_menu_system(&mut window, WindowMessage::Create, 0, 0, effects),
                WindowMsgHandled::Handled
            );
            assert_eq!(
                save_load_menu_system(&mut window, WindowMessage::Destroy, 0, 0, effects),
                WindowMsgHandled::Handled
            );
        });
    }

    #[test]
    fn save_load_menu_system_handles_glm_double_clicked_like_cpp() {
        with_test_effects(|effects| {
            let listbox_id = 101;
            let listbox_window = Rc::new(RefCell::new(GameWindow::new()));
            let mut list_box = crate::gui::gadgets::ListBox::new(listbox_id as u32, 0, 0, 200, 80);
            list_box.add_item_with_data(0, "Existing save", Some(ListBoxItemData::Integer(0)));
            assert!(list_box.select_index(0, KeyModifiers::none()));
            listbox_window
                .borrow_mut()
                .set_widget(crate::gui::WindowWidget::ListBox(list_box));

            {
                let state = effects.save_load_mut();
                *state = SaveLoadMenuState::default();
                state.listbox_games = listbox_id;
                state.listbox_games_window = Some(listbox_window.clone());
            }

            let mut window = GameWindow::new();
            assert_eq!(
                save_load_menu_system(
                    &mut window,
                    WindowMessage::User(GLM_DOUBLE_CLICKED),
                    listbox_id as WindowMsgData,
                    (-1isize) as WindowMsgData,
                    effects
                ),
                WindowMsgHandled::Handled
            );

            let owner = effects.window_manager_owner.unwrap();
            owner.complete_event_callbacks(effects);
            let selected = listbox_window
                .borrow_mut()
                .list_box_mut()
                .map(|list_box| list_box.selected_indices().to_vec())
                .unwrap_or_default();
            assert!(
                selected.is_empty(),
                "C++ GadgetListBoxSetSelected(-1) clears stale selection"
            );
        });
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
        with_test_effects(|effects| {
            clear_host_popup_save_load_bridge(effects);
            publish_host_popup_save_load_entries(
                vec![PopupSaveLoadEntry {
                    filename: "wnd_pause".into(),
                    description: "Windowed pause save".into(),
                    ..Default::default()
                }],
                effects,
            );

            assert!(effects.save_load_host().installed);
            assert_eq!(effects.save_load_host().entries.clone().len(), 1);
            assert!(take_host_popup_save_load_requests(effects).is_empty());

            dispatch_save_from_popup_confirmation(
                "wnd_pause".into(),
                "Windowed pause save".into(),
                SaveFileType::Normal,
                effects,
            );
            dispatch_load_from_popup_confirmation(
                SaveLoadSelection::Host(PopupSaveLoadEntry {
                    filename: "wnd_pause".into(),
                    description: "Windowed pause save".into(),
                    ..Default::default()
                }),
                effects,
                None,
            );

            assert_eq!(
                take_host_popup_save_load_requests(effects),
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
            clear_host_popup_save_load_bridge(effects);
        });
    }

    #[test]
    fn installed_empty_host_inventory_never_falls_back_to_common_rows() {
        with_test_effects(|effects| {
            clear_host_popup_save_load_bridge(effects);
            publish_host_popup_save_load_entries(Vec::new(), effects);

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

            populate_save_game_listbox(&mut state, Some(Vec::new()));
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

            clear_host_popup_save_load_bridge(effects);
        });
    }

    #[test]
    fn named_live_load_confirms_the_exact_host_row() {
        with_test_effects(|effects| {
            {
                clear_host_popup_save_load_bridge(effects);
                effects.with_manager(|manager, _effects| manager.reset());
                // QuitMenu opens this popup while a game is running.  The retail
                // in-game path presents LoadConfirm; the shell path instead loads
                // directly, so model the former here.
                let previous_shell_active = effects.shell_handle().is_shell_active();
                effects.shell_handle().set_shell_active(false);
                publish_host_popup_save_load_entries(
                    vec![
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
                    ],
                    effects,
                );

                assert!(
                    drive_os_wnd_popup_save_load_load_named_and_confirm_like_cpp(
                        "wnd_pause",
                        effects
                    ),
                    "the real listbox/load/confirmation widgets must accept the requested row"
                );
                assert_eq!(
                    take_host_popup_save_load_requests(effects),
                    vec![PopupSaveLoadRequest::Load {
                        filename: "wnd_pause".into(),
                    }],
                    "a named load must never fall through to the first pre-existing row"
                );

                clear_host_popup_save_load_bridge(effects);
                effects.with_manager(|manager, _effects| manager.reset());
                effects
                    .shell_handle()
                    .set_shell_active(previous_shell_active);
            }
        });
    }

    #[test]
    fn live_save_description_confirm_emits_the_host_request() {
        with_test_effects(|effects| {
            {
                clear_host_popup_save_load_bridge(effects);
                effects.with_manager(|manager, _effects| manager.reset());
                let previous_shell_active = effects.shell_handle().is_shell_active();
                effects.shell_handle().set_shell_active(false);
                // The empty published inventory is intentional: it proves the real
                // New Save Game pseudo-row opens the description confirmation rather
                // than falling back to incompatible Common rows.
                publish_host_popup_save_load_entries(Vec::new(), effects);

                assert!(prepare_live_popup_save_load_for_click(effects));
                assert!(crate::gui::dispatch_os_click_named_window(
                    "PopupSaveLoad.wnd:ButtonSave",
                    effects
                ));
                assert!(
                    effects.with_manager(|manager, _| live_popup_save_load_window_visible(
                        "PopupSaveLoad.wnd:SaveDescParent",
                        manager
                    ))
                );

                let edit_desc = effects
                    .save_load()
                    .edit_desc
                    .clone()
                    .expect("retail EntryDesc");
                edit_desc
                    .borrow_mut()
                    .text_entry_mut()
                    .expect("retail EntryDesc text widget")
                    .set_text("Windowed pause save");

                assert!(crate::gui::dispatch_os_click_named_window(
                    "PopupSaveLoad.wnd:ButtonSaveDescConfirm",
                    effects
                ));
                assert_eq!(
                    take_host_popup_save_load_requests(effects),
                    vec![PopupSaveLoadRequest::Save {
                        filename: String::new(),
                        description: "Windowed pause save".into(),
                        save_file_type: SaveFileType::Normal,
                    }]
                );

                clear_host_popup_save_load_bridge(effects);
                effects.with_manager(|manager, _effects| manager.reset());
                effects
                    .shell_handle()
                    .set_shell_active(previous_shell_active);
            }
        });
    }

    #[test]
    fn popup_confirmation_provenance_is_snapshotted_and_other_paths_fail_closed() {
        with_test_effects(|effects| {
            clear_host_popup_save_load_bridge(effects);
            publish_host_popup_save_load_entries(
                vec![PopupSaveLoadEntry {
                    filename: "wnd_pause".into(),
                    description: "Windowed pause save".into(),
                    ..Default::default()
                }],
                effects,
            );

            // No synchronous WND scope: even an explicit confirmation is unknown.
            dispatch_save_from_popup_confirmation(
                "wnd_pause".into(),
                "Windowed pause save".into(),
                SaveFileType::Normal,
                effects,
            );
            {
                effects.input_provenance = HostControlBarInputProvenance::InjectedOrUnknown;
                dispatch_load_from_popup_confirmation(
                    SaveLoadSelection::Host(PopupSaveLoadEntry {
                        filename: "wnd_pause".into(),
                        description: "Windowed pause save".into(),
                        ..Default::default()
                    }),
                    effects,
                    None,
                );
            }
            {
                effects.input_provenance = HostControlBarInputProvenance::PhysicalWindowMouseInput;
                // Shell/double-click behavior remains valid, but has no
                // explicit confirmation provenance even under a real WND scope.
                dispatch_load_without_popup_confirmation(
                    SaveLoadSelection::Host(PopupSaveLoadEntry {
                        filename: "wnd_pause".into(),
                        description: "Windowed pause save".into(),
                        ..Default::default()
                    }),
                    effects,
                    None,
                );
                dispatch_save_from_popup_confirmation(
                    "wnd_pause".into(),
                    "Windowed pause save".into(),
                    SaveFileType::Normal,
                    effects,
                );
                dispatch_load_from_popup_confirmation(
                    SaveLoadSelection::Host(PopupSaveLoadEntry {
                        filename: "wnd_pause".into(),
                        description: "Windowed pause save".into(),
                        ..Default::default()
                    }),
                    effects,
                    None,
                );
            }

            let provenance: Vec<_> = take_host_popup_save_load_published_requests(effects)
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

            clear_host_popup_save_load_bridge(effects);
        });
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
    // List-box notifications call this only after their selected widget loan
    // ends; direct confirmation callbacks borrow a different source window.
    let mut listbox_guard = listbox.borrow_mut();
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
fn enable_window_rc(
    window: &Rc<RefCell<GameWindow>>,
    enabled: bool,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    let window = Rc::downgrade(window);
    effects.queue_window_operation(move |manager, _effects| {
        let Some(window) = window.upgrade() else {
            return;
        };
        let id = window.borrow().get_id();
        let Some(current) = manager.get_window_by_id(id) else {
            return;
        };
        if !Rc::ptr_eq(&current, &window) {
            return;
        }
        let _ = window.borrow_mut().enable(enabled);
    });
}

/// Apply visibility after the current callback releases its selected window.
/// Validate the original window identity before touching focus or modal state.
fn hide_save_load_window(
    window: &Rc<RefCell<GameWindow>>,
    hide: bool,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    let window = Rc::downgrade(window);
    effects.queue_window_operation(move |manager, _effects| {
        let Some(window) = window.upgrade() else {
            return;
        };
        let id = window.borrow().get_id();
        let Some(current) = manager.get_window_by_id(id) else {
            return;
        };
        if !Rc::ptr_eq(&current, &window) {
            return;
        }
        let _ = manager.hide_window(&window, hide);
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

fn update_menu_actions(effects: &mut crate::gui::game_window::WindowEventEffects<'_>) {
    let can_save = effects.save_load().current_layout_type != SaveLoadLayoutType::LoadOnly;
    if let Some(save_button) = effects.save_load().button_save_window.clone().as_ref() {
        enable_window_rc(save_button, can_save, effects);
    }

    let selected = selected_save(effects.save_load());
    let has_selection = selected.is_some();
    if let Some(load_button) = effects.save_load().button_load_window.clone().as_ref() {
        enable_window_rc(load_button, has_selection, effects);
    }
    if let Some(delete_button) = effects.save_load().button_delete_window.clone().as_ref() {
        // Host rows can be loaded/saved through the typed host bridge, but
        // this callback has no host delete request. Do not make a live button
        // claim an operation we cannot safely perform on the host format.
        enable_window_rc(
            delete_button,
            matches!(selected, Some(SaveLoadSelection::Common(_))),
            effects,
        );
    }
}

fn close_save_menu(
    window: &mut GameWindow,
    is_popup: bool,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    if is_popup {
        if let Some(layout) = window.get_layout() {
            effects.with_manager(|manager, _effects| {
                layout
                    .borrow()
                    .hide_with_manager_and_selected(manager, true, Some(window))
            });
        }
    } else {
        queue_shell_hide(effects);
    }
}

/// Deferred listbox actions run after the originating window's input borrow
/// has unwound. At that point the parsed parent can safely supply its layout
/// for the same popup close operation as `close_save_menu`.
fn close_save_menu_after_dispatch(effects: &mut crate::gui::game_window::WindowEventEffects<'_>) {
    if effects.save_load().is_popup {
        if let Some(layout) = effects
            .save_load()
            .parent
            .as_ref()
            .map(|parent| parent.borrow())
            .and_then(|parent| parent.get_layout())
        {
            effects.with_manager(|manager, _effects| layout.borrow().hide(manager, true));
        }
    } else {
        queue_shell_hide(effects);
    }
}

fn do_load_game(
    selected: AvailableGameInfo,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    mut selected_window: Option<&mut crate::gui::GameWindow>,
) {
    let shell_active = effects.shell_handle().is_shell_active();
    if !shell_active {
        destroy_quit_menu(effects);
    } else {
        effects.with_manager(|manager, effects| {
            manager.transition_remove(
                "MainMenuLoadReplayMenu",
                false,
                selected_window.as_deref_mut(),
            );
            manager.transition_remove(
                "MainMenuLoadReplayMenuBack",
                false,
                selected_window.as_deref_mut(),
            );
        });
        TheGameLogic::prepare_new_game(GAME_SINGLE_PLAYER, DIFFICULTY_NORMAL, 0);
    }

    let load_result = {
        let mut game_state = get_game_state();
        effects.ui_mut().with_borrowed_ui_snapshots(|snapshots| {
            game_state.load_game_with_snapshots(selected, snapshots)
        })
    };
    if !matches!(load_result, Ok(SaveCode::Ok)) {
        if TheGameLogic::is_in_game() {
            let _ = TheGameLogic::clear_game_data();
        }
        if let Some(engine) = get_game_engine() {
            let mut engine = engine.lock();
            let _ = pollster::block_on(engine.reset());
        }
        queue_shell_show(effects, true);
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
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    if queue_host_popup_save_load_request(
        PopupSaveLoadRequest::Save {
            filename: filename.clone(),
            description: description.clone(),
            save_file_type,
        },
        input_provenance,
        effects,
    ) {
        return;
    }

    let mut game_state = get_game_state();
    let _ = effects.ui_mut().with_borrowed_ui_snapshots(|snapshots| {
        game_state.save_game_with_snapshots(
            filename,
            description,
            save_file_type,
            SnapshotType::SaveLoad,
            snapshots,
        )
    });
}

/// Dispatch a Load through the active host bridge when present.
/// Common `TheGameState` is only used when no host bridge is installed.
fn dispatch_load_request(
    selected: SaveLoadSelection,
    input_provenance: HostControlBarInputProvenance,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    mut selected_window: Option<&mut crate::gui::GameWindow>,
) {
    if queue_host_popup_save_load_request(
        PopupSaveLoadRequest::Load {
            filename: selected.filename().to_string(),
        },
        input_provenance,
        effects,
    ) {
        return;
    }

    if let Some(common) = selected.into_common() {
        do_load_game(common, effects, selected_window.as_deref_mut());
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
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    dispatch_save_request(
        filename,
        description,
        save_file_type,
        effects.input_provenance(),
        effects,
    );
}

/// Dispatch work from `ButtonLoadConfirm`, retaining its exact WND provenance.
fn dispatch_load_from_popup_confirmation(
    selected: SaveLoadSelection,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    mut selected_window: Option<&mut crate::gui::GameWindow>,
) {
    dispatch_load_request(
        selected,
        effects.input_provenance(),
        effects,
        selected_window.as_deref_mut(),
    );
}

/// Dispatch a legitimate Popup load path which did not pass through
/// `ButtonLoadConfirm` (the retail shell's immediate load or list double-click).
/// It must retain normal load behavior, but cannot serve as confirmation proof.
fn dispatch_load_without_popup_confirmation(
    selected: SaveLoadSelection,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    mut selected_window: Option<&mut crate::gui::GameWindow>,
) {
    dispatch_load_request(
        selected,
        HostControlBarInputProvenance::InjectedOrUnknown,
        effects,
        selected_window.as_deref_mut(),
    );
}

/// Apply the C++ Load-button UI transition.  A shell load completes immediately
/// in the original game; callers must dispatch the returned selection only
/// after releasing the menu state loan.
fn process_load_button_press(
    window: &mut GameWindow,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> Option<SaveLoadSelection> {
    let selected = selected_save(effects.save_load())?;

    if effects.shell_handle().is_shell_active() {
        close_save_menu(window, effects.save_load().is_popup, effects);
        return Some(selected);
    }

    if let Some(listbox) = effects.save_load().listbox_games_window.clone().as_ref() {
        enable_window_rc(listbox, false, effects);
    }
    if let Some(frame) = effects.save_load().button_frame.clone().as_ref() {
        enable_window_rc(frame, false, effects);
    }
    if let Some(confirm) = effects.save_load().load_confirm.clone().as_ref() {
        hide_save_load_window(confirm, false, effects);
    }
    None
}

/// Finish a listbox double-click after `GameWindow` has released its mutable
/// listbox borrow. The normal ButtonLoad path can run synchronously; a listbox
/// event cannot, because querying its selected row during dispatch would be a
/// `RefCell` re-entry.
fn process_load_double_click_after_dispatch(
    row_selected: i32,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> Option<SaveLoadSelection> {
    let listbox = effects.save_load().listbox_games_window.clone()?;
    let mut listbox = listbox.borrow_mut();
    let listbox_widget = listbox.list_box_mut()?;
    set_listbox_selection_from_cpp_row(listbox_widget, row_selected);
    drop(listbox);

    let selected = selected_save(effects.save_load())?;
    if effects.shell_handle().is_shell_active() {
        close_save_menu_after_dispatch(effects);
        return Some(selected);
    }

    if let Some(listbox) = effects.save_load().listbox_games_window.clone().as_ref() {
        enable_window_rc(listbox, false, effects);
    }
    if let Some(frame) = effects.save_load().button_frame.clone().as_ref() {
        enable_window_rc(frame, false, effects);
    }
    if let Some(confirm) = effects.save_load().load_confirm.clone().as_ref() {
        hide_save_load_window(confirm, false, effects);
    }
    None
}

fn queue_menu_actions_refresh_after_dispatch(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    let generation = effects.save_load().binding_generation;
    effects.queue_window_operation(move |_manager, effects| {
        if effects.save_load().binding_generation != generation {
            return;
        }
        update_menu_actions(effects);
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

fn process_delete_confirmed(effects: &mut crate::gui::game_window::WindowEventEffects<'_>) {
    let selected = selected_common_game_info(selected_save(effects.save_load()));
    if let Some(selected) = selected {
        let game_state = get_game_state();
        let filepath = game_state.get_file_path_in_save_directory(&selected.filename);
        let _ = fs::remove_file(filepath);
    }
    populate_receiving_save_game_listbox(effects);
}

pub(crate) fn reset_save_load_menu(effects: &mut crate::gui::game_window::WindowEventEffects<'_>) {
    let generation = effects.save_load().binding_generation.wrapping_add(1);
    *effects.save_load_mut() = SaveLoadMenuState {
        binding_generation: generation,
        ..Default::default()
    };
    effects.save_load_host_mut().clear();
}

pub fn save_load_menu_init(
    layout: &WindowLayout,
    user_data: Option<&dyn std::any::Any>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    effects.save_load_mut().current_layout_type = SaveLoadLayoutType::SaveAndLoad;
    effects.save_load_mut().is_popup = true;
    if let Some(layout_type) = user_data.and_then(|data| data.downcast_ref::<SaveLoadLayoutType>())
    {
        effects.save_load_mut().current_layout_type = *layout_type;
    }

    init_gadget_ids(effects.save_load_mut(), "PopupSaveLoad.wnd");
    let parent_id = NameKeyGenerator::name_to_key("PopupSaveLoad.wnd:SaveLoadMenu") as i32;
    load_controls(
        effects.save_load_mut(),
        layout,
        parent_id,
        "PopupSaveLoad.wnd",
    );

    if let Some(frame) = effects.save_load().button_frame.clone().as_ref() {
        enable_window_rc(frame, true, effects);
    }
    if let Some(window) = effects.save_load().overwrite_confirm.clone().as_ref() {
        hide_save_load_window(window, true, effects);
    }
    if let Some(window) = effects.save_load().load_confirm.clone().as_ref() {
        hide_save_load_window(window, true, effects);
    }
    if let Some(window) = effects.save_load().save_desc.clone().as_ref() {
        hide_save_load_window(window, true, effects);
    }
    if let Some(window) = effects.save_load().delete_confirm.clone().as_ref() {
        hide_save_load_window(window, true, effects);
    }

    populate_receiving_save_game_listbox(effects);
    update_menu_actions(effects);

    // Do not call back into WindowManager while this callback may itself be
    // running under WindowManager input dispatch. Queue the focus/modal work
    // after releasing the menu state loan; the retail parent is already in
    // the parsed WND tree.
    let parent = effects.save_load().parent.clone();
    if let Some(parent) = parent {
        queue_save_load_focus(parent.clone(), effects);
        effects.queue_window_operation(move |manager, effects| {
            let _ = manager.set_modal(parent);
        });
    }
}

pub fn save_load_menu_full_screen_init(
    layout: &WindowLayout,
    user_data: Option<&dyn std::any::Any>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    show_shell_map_if_available(true, effects);

    effects.save_load_mut().is_popup = false;
    effects.save_load_mut().current_layout_type = SaveLoadLayoutType::LoadOnly;
    if let Some(layout_type) = user_data.and_then(|data| data.downcast_ref::<SaveLoadLayoutType>())
    {
        effects.save_load_mut().current_layout_type = *layout_type;
    }

    init_gadget_ids(effects.save_load_mut(), "SaveLoad.wnd");
    let parent_id = NameKeyGenerator::name_to_key("SaveLoad.wnd:SaveLoadMenu") as i32;
    load_controls(effects.save_load_mut(), layout, parent_id, "SaveLoad.wnd");

    if let Some(frame) = effects.save_load().button_frame.clone().as_ref() {
        enable_window_rc(frame, true, effects);
    }
    if let Some(window) = effects.save_load().overwrite_confirm.clone().as_ref() {
        hide_save_load_window(window, true, effects);
    }
    if let Some(window) = effects.save_load().load_confirm.clone().as_ref() {
        hide_save_load_window(window, true, effects);
    }
    if let Some(window) = effects.save_load().save_desc.clone().as_ref() {
        hide_save_load_window(window, true, effects);
    }
    if let Some(window) = effects.save_load().delete_confirm.clone().as_ref() {
        hide_save_load_window(window, true, effects);
    }

    populate_receiving_save_game_listbox(effects);
    update_menu_actions(effects);

    effects.with_manager(|manager, _effects| layout.hide(manager, false));
    effects.save_load_mut().just_entered = true;
    effects.save_load_mut().initial_gadget_delay = 2;
    if let Some(parent) = effects.save_load().parent.clone().as_ref() {
        hide_save_load_window(parent, true, effects);
    }
    effects.save_load_mut().is_shutting_down = false;

    let parent = effects.save_load().parent.clone();
    if let Some(parent) = parent {
        queue_save_load_focus(parent, effects);
    }
}

pub fn save_load_menu_shutdown(
    layout: &WindowLayout,
    user_data: Option<&dyn std::any::Any>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    let pop_immediate = user_data
        .and_then(|data| data.downcast_ref::<bool>())
        .copied()
        .unwrap_or(false);

    if pop_immediate {
        effects.with_manager(|manager, _effects| layout.hide(manager, true));
        queue_shell_shutdown_complete(effects, false);
        return;
    }

    effects.with_manager(|manager, effects| {
        manager.transition_reverse("SaveLoadMenuFade", None);
    });
    effects.save_load_mut().is_shutting_down = true;
}

pub fn save_load_menu_update(
    layout: &WindowLayout,
    _user_data: Option<&dyn std::any::Any>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    if get_dont_show_main_menu(effects) && effects.save_load().just_entered {
        effects.save_load_mut().just_entered = false;
    }
    if get_replay_was_pressed(effects) && effects.save_load().just_entered {
        effects.save_load_mut().just_entered = false;
        set_replay_was_pressed(effects, false);
    }

    if effects.save_load().just_entered {
        if effects.save_load().initial_gadget_delay == 1 {
            effects.with_manager(|manager, effects| {
                manager.transition_remove("MainMenuDefaultMenuLogoFade", false, None);
                manager.transition_set_group("SaveLoadMenuFade", false, None);
            });
            effects.save_load_mut().initial_gadget_delay = 2;
            effects.save_load_mut().just_entered = false;
        } else {
            effects.save_load_mut().initial_gadget_delay -= 1;
        }
    }

    if effects.save_load().is_shutting_down {
        let shell_finished = crate::gui::shell::base::shell_anim_finished_for_layout(effects);
        let transitions_finished =
            effects.with_manager(|manager, effects| manager.transitions_finished());
        if shell_finished && transitions_finished {
            effects.with_manager(|manager, _effects| layout.hide(manager, true));
            queue_shell_shutdown_complete(effects, false);
        }
    }
}

pub fn save_load_menu_input(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    if msg != WindowMessage::Char || data1 != KEY_ESC {
        return WindowMsgHandled::Ignored;
    }
    if (data2 & KEY_STATE_UP) == 0 {
        return WindowMsgHandled::Handled;
    }

    if let Some(confirm) = effects.save_load().delete_confirm.clone().as_ref() {
        hide_save_load_window(confirm, true, effects);
    }
    if let Some(listbox) = effects.save_load().listbox_games_window.clone().as_ref() {
        enable_window_rc(listbox, true, effects);
    }
    if let Some(frame) = effects.save_load().button_frame.clone().as_ref() {
        enable_window_rc(frame, true, effects);
    }

    let button_back = effects.save_load().button_back;
    let _ = window.send_system_message(
        WindowMessage::GadgetSelected,
        button_back as WindowMsgData,
        0,
        effects,
    );

    WindowMsgHandled::Handled
}

pub fn save_load_menu_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::Create | WindowMessage::Destroy => WindowMsgHandled::Handled,
        WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
        WindowMessage::User(code) if code == GLM_DOUBLE_CLICKED => {
            if data1 as i32 == effects.save_load().listbox_games {
                let row_selected = data2 as i32;
                let generation = effects.save_load().binding_generation;
                effects.queue_window_operation(move |manager, effects| {
                    effects.with_receiving_manager(manager, |effects| {
                        if effects.save_load().binding_generation != generation {
                            return;
                        }
                        let selected =
                            process_load_double_click_after_dispatch(row_selected, effects);
                        if let Some(selected) = selected {
                            dispatch_load_without_popup_confirmation(selected, effects, None);
                        }
                    });
                });
                return WindowMsgHandled::Handled;
            }
            WindowMsgHandled::Ignored
        }
        WindowMessage::GadgetSelected => {
            let control_id = data1 as i32;
            if control_id == effects.save_load().listbox_games {
                // The listbox itself is still mutably borrowed by input
                // dispatch. Refresh its sibling action buttons after the
                // borrow unwinds, rather than falsely treating the selection
                // as absent or panicking on a re-entrant RefCell borrow.
                queue_menu_actions_refresh_after_dispatch(effects);
                return WindowMsgHandled::Handled;
            }

            if control_id == effects.save_load().button_load {
                if let Some(selected) = process_load_button_press(window, effects) {
                    dispatch_load_without_popup_confirmation(selected, effects, Some(&mut *window));
                }
                return WindowMsgHandled::Handled;
            }

            if control_id == effects.save_load().button_save {
                let selected = selected_save(effects.save_load());
                if selected.is_none() {
                    if let Some(save_desc) = effects.save_load().save_desc.clone().as_ref() {
                        hide_save_load_window(save_desc, false, effects);
                    }
                    if let Some(edit_desc) = effects.save_load().edit_desc.clone().as_ref() {
                        set_edit_description(edit_desc, effects.campaign_manager.as_deref());
                        queue_save_load_focus(edit_desc.clone(), effects);
                    }
                    if let Some(listbox) = effects.save_load().listbox_games_window.clone().as_ref()
                    {
                        enable_window_rc(listbox, false, effects);
                    }
                } else {
                    if let Some(listbox) = effects.save_load().listbox_games_window.clone().as_ref()
                    {
                        enable_window_rc(listbox, false, effects);
                    }
                    if let Some(frame) = effects.save_load().button_frame.clone().as_ref() {
                        enable_window_rc(frame, false, effects);
                    }
                    if let Some(confirm) = effects.save_load().overwrite_confirm.clone().as_ref() {
                        hide_save_load_window(confirm, false, effects);
                    }
                }
                return WindowMsgHandled::Handled;
            }

            if control_id == effects.save_load().button_delete {
                let selected = selected_save(effects.save_load());
                if should_show_delete_for_selection(&selected) {
                    if let Some(listbox) = effects.save_load().listbox_games_window.clone().as_ref()
                    {
                        enable_window_rc(listbox, false, effects);
                    }
                    if let Some(frame) = effects.save_load().button_frame.clone().as_ref() {
                        enable_window_rc(frame, false, effects);
                    }
                    if let Some(confirm) = effects.save_load().delete_confirm.clone().as_ref() {
                        hide_save_load_window(confirm, false, effects);
                    }
                }
                return WindowMsgHandled::Handled;
            }

            if control_id == effects.save_load().button_back {
                if effects.save_load().is_popup {
                    close_save_menu(window, true, effects);
                } else {
                    queue_shell_pop(effects);
                }
                return WindowMsgHandled::Handled;
            }

            if control_id == effects.save_load().button_delete_confirm
                || control_id == effects.save_load().button_delete_cancel
            {
                if control_id == effects.save_load().button_delete_confirm {
                    process_delete_confirmed(effects);
                }

                if let Some(confirm) = effects.save_load().delete_confirm.clone().as_ref() {
                    hide_save_load_window(confirm, true, effects);
                }
                if let Some(listbox) = effects.save_load().listbox_games_window.clone().as_ref() {
                    enable_window_rc(listbox, true, effects);
                }
                if let Some(frame) = effects.save_load().button_frame.clone().as_ref() {
                    enable_window_rc(frame, true, effects);
                }
                update_menu_actions(effects);
                return WindowMsgHandled::Handled;
            }

            if control_id == effects.save_load().button_overwrite_cancel
                || control_id == effects.save_load().button_overwrite_confirm
            {
                if let Some(confirm) = effects.save_load().overwrite_confirm.clone().as_ref() {
                    hide_save_load_window(confirm, true, effects);
                }

                if control_id == effects.save_load().button_overwrite_confirm {
                    if let Some(listbox) = effects.save_load().listbox_games_window.clone().as_ref()
                    {
                        enable_window_rc(listbox, true, effects);
                    }
                    if let Some(frame) = effects.save_load().button_frame.clone().as_ref() {
                        enable_window_rc(frame, true, effects);
                    }
                    update_menu_actions(effects);
                    close_save_menu(window, effects.save_load().is_popup, effects);

                    let file_type =
                        save_file_type_for_layout(effects.save_load().current_layout_type);
                    let (filename, desc) =
                        selected_filename_and_description(selected_save(effects.save_load()));
                    // Saving can serialize engine effects.save_load() and is not part of
                    // the UI-effects.save_load() critical section. Release it before the
                    // real save so nested UI/engine work cannot deadlock the
                    // menu state borrow during a button dispatch.
                    dispatch_save_from_popup_confirmation(filename, desc, file_type, effects);
                    return WindowMsgHandled::Handled;
                } else {
                    if let Some(frame) = effects.save_load().button_frame.clone().as_ref() {
                        enable_window_rc(frame, true, effects);
                    }
                    update_menu_actions(effects);
                    if let Some(listbox) = effects.save_load().listbox_games_window.clone().as_ref()
                    {
                        enable_window_rc(listbox, true, effects);
                    }
                }

                return WindowMsgHandled::Handled;
            }

            if control_id == effects.save_load().button_save_desc_confirm {
                let desc = effects
                    .save_load()
                    .edit_desc
                    .as_ref()
                    .and_then(|edit| {
                        let mut edit = edit.borrow_mut();
                        edit.text_entry_mut().map(|entry| entry.text().to_string())
                    })
                    .unwrap_or_default();

                if let Some(save_desc) = effects.save_load().save_desc.clone().as_ref() {
                    hide_save_load_window(save_desc, true, effects);
                }
                if let Some(listbox) = effects.save_load().listbox_games_window.clone().as_ref() {
                    enable_window_rc(listbox, true, effects);
                }
                if let Some(frame) = effects.save_load().button_frame.clone().as_ref() {
                    enable_window_rc(frame, true, effects);
                }
                update_menu_actions(effects);
                close_save_menu(window, effects.save_load().is_popup, effects);

                let (filename, _) =
                    selected_filename_and_description(selected_save(effects.save_load()));
                let file_type = save_file_type_for_layout(effects.save_load().current_layout_type);
                // See overwrite-confirm above: serialize only after dropping
                // the callback's menu state loan.
                dispatch_save_from_popup_confirmation(filename, desc, file_type, effects);
                return WindowMsgHandled::Handled;
            }

            if control_id == effects.save_load().button_save_desc_cancel {
                if let Some(save_desc) = effects.save_load().save_desc.clone().as_ref() {
                    hide_save_load_window(save_desc, true, effects);
                }
                if let Some(listbox) = effects.save_load().listbox_games_window.clone().as_ref() {
                    enable_window_rc(listbox, true, effects);
                }
                if let Some(frame) = effects.save_load().button_frame.clone().as_ref() {
                    enable_window_rc(frame, true, effects);
                }
                update_menu_actions(effects);
                return WindowMsgHandled::Handled;
            }

            if control_id == effects.save_load().button_load_confirm
                || control_id == effects.save_load().button_load_cancel
            {
                if let Some(confirm) = effects.save_load().load_confirm.clone().as_ref() {
                    hide_save_load_window(confirm, true, effects);
                }
                if let Some(listbox) = effects.save_load().listbox_games_window.clone().as_ref() {
                    enable_window_rc(listbox, true, effects);
                }
                if let Some(frame) = effects.save_load().button_frame.clone().as_ref() {
                    enable_window_rc(frame, true, effects);
                }
                update_menu_actions(effects);

                let selected = (control_id == effects.save_load().button_load_confirm)
                    .then(|| selected_save(effects.save_load()))
                    .flatten();
                if let Some(selected) = selected {
                    close_save_menu(window, effects.save_load().is_popup, effects);
                    // Loading destroys/rebuilds game effects.save_load(). It must not hold
                    // the UI menu state borrow while it tears down QuitMenu/layouts.
                    dispatch_load_from_popup_confirmation(selected, effects, Some(&mut *window));
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

fn residual_action_store(
    action: ResidualSaveLoadAction,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    effects.save_load_mut().residual_action = action as u8;
}

/// Residual: last SaveLoad residual action.
pub fn residual_save_load_last_action(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> ResidualSaveLoadAction {
    match effects.save_load().residual_action {
        1 => ResidualSaveLoadAction::SelectSlot,
        2 => ResidualSaveLoadAction::Load,
        3 => ResidualSaveLoadAction::Save,
        4 => ResidualSaveLoadAction::Delete,
        5 => ResidualSaveLoadAction::Back,
        _ => ResidualSaveLoadAction::None,
    }
}

/// Residual: last selected slot index (-1 if none).
pub fn residual_save_load_selected_slot(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> Option<i32> {
    effects.save_load().residual_slot
}

/// Residual: bind SaveLoad gadget IDs for popup or full-screen layout.
pub fn simulate_save_load_menu_bind_layout(
    popup: bool,
    layout_type: SaveLoadLayoutType,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let prefix = if popup {
        SAVE_LOAD_LAYOUT_PREFIX_POPUP
    } else {
        SAVE_LOAD_LAYOUT_PREFIX_FULL
    };
    init_gadget_ids(effects.save_load_mut(), prefix);
    effects.save_load_mut().is_popup = popup;
    effects.save_load_mut().current_layout_type = layout_type;
    effects.save_load_mut().is_shutting_down = false;
    effects.save_load().button_load != 0
        || effects.save_load().button_save != 0
        || effects.save_load().button_back != 0
        || effects.save_load().listbox_games != 0
}

/// Residual: select a listbox games slot index (no live listbox widget required).
pub fn simulate_save_load_menu_select_slot(
    slot_index: i32,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if slot_index < 0 {
        return false;
    }
    if effects.save_load().listbox_games == 0 {
        init_gadget_ids(effects.save_load_mut(), SAVE_LOAD_LAYOUT_PREFIX_FULL);
    }
    effects.save_load_mut().residual_slot = Some(slot_index);
    residual_action_store(ResidualSaveLoadAction::SelectSlot, effects);
    true
}

/// Residual: fire ButtonLoad without full do_load_game (asset/engine reset).
/// Requires a prior select_slot residual. Latches Load action honesty.
pub fn simulate_save_load_menu_load_button_gadget_selected(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if effects.save_load().button_load == 0 {
        init_gadget_ids(effects.save_load_mut(), SAVE_LOAD_LAYOUT_PREFIX_FULL);
    }
    if residual_save_load_selected_slot(effects).is_none() {
        // C++ ignores Load with no selection.
        return false;
    }
    // Layout type LoadOnly / SaveAndLoad can load; SaveOnly cannot.
    if matches!(
        effects.save_load().current_layout_type,
        SaveLoadLayoutType::SaveOnly
    ) {
        return false;
    }
    residual_action_store(ResidualSaveLoadAction::Load, effects);
    true
}

/// Residual: fire ButtonSave without full save file write.
pub fn simulate_save_load_menu_save_button_gadget_selected(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if effects.save_load().button_save == 0 {
        init_gadget_ids(effects.save_load_mut(), SAVE_LOAD_LAYOUT_PREFIX_FULL);
    }
    if matches!(
        effects.save_load().current_layout_type,
        SaveLoadLayoutType::LoadOnly
    ) {
        return false;
    }
    residual_action_store(ResidualSaveLoadAction::Save, effects);
    true
}

/// Residual: fire ButtonDelete without filesystem remove.
pub fn simulate_save_load_menu_delete_button_gadget_selected(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if effects.save_load().button_delete == 0 {
        init_gadget_ids(effects.save_load_mut(), SAVE_LOAD_LAYOUT_PREFIX_FULL);
    }
    if residual_save_load_selected_slot(effects).is_none() {
        return false;
    }
    residual_action_store(ResidualSaveLoadAction::Delete, effects);
    true
}

/// Residual: fire ButtonBack (shell pop / popup hide residual latch).
pub fn simulate_save_load_menu_back_button_gadget_selected(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if effects.save_load().button_back == 0 {
        init_gadget_ids(effects.save_load_mut(), SAVE_LOAD_LAYOUT_PREFIX_FULL);
    }
    residual_action_store(ResidualSaveLoadAction::Back, effects);
    effects.save_load_mut().residual_slot = None;
    true
}

/// Live WindowManager lookup for a `PopupSaveLoad.wnd:*` gadget.
///
/// Returns true only when the named window exists in the widget tree.
/// Residual `simulate_*` latches do **not** count.
pub fn live_popup_save_load_window_present(
    name: &str,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) -> bool {
    (|manager: &mut crate::gui::window_manager::WindowManager| {
        manager.find_window_by_name(name).is_some()
    })(receiving_manager)
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

fn live_popup_save_load_layout(
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) -> Option<Rc<RefCell<WindowLayout>>> {
    (|manager: &mut crate::gui::window_manager::WindowManager| {
        manager
            .find_window_by_name(POPUP_SAVE_LOAD_PARENT)
            .and_then(|parent| parent.borrow().get_layout())
    })(receiving_manager)
}

fn retail_popup_save_load_controls_present(
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) -> bool {
    (|manager: &mut crate::gui::window_manager::WindowManager| {
        POPUP_SAVE_LOAD_RETAIL_CONTROLS
            .iter()
            .all(|name| manager.find_window_by_name(name).is_some())
    })(receiving_manager)
}

fn popup_save_load_state_is_bound_to(
    layout: &Rc<RefCell<WindowLayout>>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let Some(parent) = layout_window_by_id(
        &layout.borrow(),
        NameKeyGenerator::name_to_key(POPUP_SAVE_LOAD_PARENT) as i32,
    ) else {
        return false;
    };
    effects.save_load().is_popup
        && effects
            .save_load()
            .parent
            .as_ref()
            .is_some_and(|bound_parent| Rc::ptr_eq(bound_parent, &parent))
        && effects.save_load().listbox_games_window.is_some()
}

fn hide_popup_save_load_confirmations(
    layout: &Rc<RefCell<WindowLayout>>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
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
            hide_save_load_window(&window, true, effects);
        }
    }
}

/// Show a previously parsed popup while retaining the retail confirmation
/// windows' hidden state. `WindowLayout::hide(false)` restores every child, so
/// the confirmation parents must be hidden after that queued layout operation.
pub(crate) fn show_live_popup_save_load_layout(
    layout: Rc<RefCell<WindowLayout>>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    effects.with_manager(|manager, _effects| layout.borrow().hide(manager, false));
    effects.with_manager(|manager, _effects| layout.borrow_mut().bring_forward(manager));
    effects.queue_window_operation(move |_manager, effects| {
        hide_popup_save_load_confirmations(&layout, effects);
    });
}

/// Create and initialize the retail `Menus/PopupSaveLoad.wnd` layout if it is
/// missing. There is deliberately no synthetic-widget fallback: the real WND
/// includes the listbox, confirmation dialogs, and callbacks a usable menu
/// requires.
pub fn ensure_live_popup_save_load_layout(
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let layout = match live_popup_save_load_layout(receiving_manager) {
        Some(layout) => layout,
        None => match (|manager: &mut crate::gui::window_manager::WindowManager| {
            manager
                .create_layout_with_windows("Menus/PopupSaveLoad.wnd", effects)
                .ok()
                .map(|(layout, _)| layout)
        })(receiving_manager)
        {
            Some(layout) => layout,
            None => return false,
        },
    };

    if !retail_popup_save_load_controls_present(receiving_manager) {
        return false;
    }
    if !popup_save_load_state_is_bound_to(&layout, effects) {
        effects.with_receiving_manager(receiving_manager, |effects| {
            layout.borrow().run_init_with_effects(None, effects);
        });
    }

    retail_popup_save_load_controls_present(receiving_manager)
        && popup_save_load_state_is_bound_to(&layout, effects)
}

/// Show the fully parsed popup for an OS click. This preserves the C++ button
/// state: Load remains disabled when no real save-game row is selected.
pub fn prepare_live_popup_save_load_for_click(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if !effects.with_manager(|receiving_manager, effects| {
        ensure_live_popup_save_load_layout(receiving_manager, effects)
    }) {
        return false;
    }
    let Some(layout) = effects
        .with_manager(|receiving_manager, effects| live_popup_save_load_layout(receiving_manager))
    else {
        return false;
    };
    // Reopening a hidden popup mirrors QuitMenuSystem's `run_init` call and
    // restores focus/modal state after `close_save_menu` removed it. Avoid a
    // second modal push while an already initialized popup is visible.
    if layout.borrow().is_hidden() || !popup_save_load_state_is_bound_to(&layout, effects) {
        layout.borrow().run_init_with_effects(None, effects);
    }
    show_live_popup_save_load_layout(layout, effects);
    effects.with_manager(|receiving_manager, effects| {
        retail_popup_save_load_controls_present(receiving_manager)
    })
}

fn live_popup_save_load_window_visible(
    name: &str,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) -> bool {
    (|manager: &mut crate::gui::window_manager::WindowManager| {
        manager.find_window_by_name(name).is_some_and(|window| {
            let window = window.borrow();
            !window.is_hidden() && window.is_enabled()
        })
    })(receiving_manager)
}

/// Select a genuine save-game entry in the retail listbox and send the same
/// parent `GadgetSelected` notification that updates the C++ action buttons.
fn select_live_popup_save_game_matching_like_cpp(
    predicate: impl Fn(&SaveLoadSelection) -> bool,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if !prepare_live_popup_save_load_for_click(effects) {
        return false;
    }

    let Some(listbox_window) = effects
        .save_load()
        .listbox_games_window
        .clone()
        .as_ref()
        .cloned()
    else {
        return false;
    };
    let Some(parent) = effects.save_load().parent.clone().as_ref().cloned() else {
        return false;
    };
    let listbox_id = effects.save_load().listbox_games;

    let selected = {
        let mut listbox_window = listbox_window.borrow_mut();
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
                let selection = if effects.save_load().host_bridge_active {
                    effects
                        .save_load()
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
    if !selected {
        return false;
    }

    let handled = parent
        .borrow_mut()
        .send_system_message(
            WindowMessage::GadgetSelected,
            listbox_id as WindowMsgData,
            0,
            effects,
        )
        .is_handled();
    handled
}

/// Select the first genuine save-game entry in the retail listbox.
///
/// The popup always places its `New Save Game` pseudo-row first, whose item
/// data is `-1`; it must never be used for a Load action.
pub fn select_first_live_popup_save_game_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    select_live_popup_save_game_matching_like_cpp(|_| true, effects)
}

/// Select the exact filename published in the current PopupSaveLoad list.
/// This is required for a host round-trip: loading the first row could select
/// an unrelated pre-existing user save.
pub fn select_live_popup_save_game_named_like_cpp(
    filename: &str,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if filename.is_empty() {
        return false;
    }
    select_live_popup_save_game_matching_like_cpp(
        |selection| selection.filename() == filename,
        effects,
    )
}

/// OS click on live `PopupSaveLoad.wnd:ButtonSave` (not `simulate_*`).
pub fn drive_os_wnd_popup_save_load_save_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let _ = prepare_live_popup_save_load_for_click(effects);
    crate::gui::dispatch_os_click_named_window("PopupSaveLoad.wnd:ButtonSave", effects)
}

/// Complete the retail save interaction: Save then the visible description or
/// overwrite confirmation. The actual `SaveLoadMenuSystem` callback performs
/// the GameState save; this helper only drives its live WND buttons.
pub fn drive_os_wnd_popup_save_load_save_and_confirm_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if !drive_os_wnd_popup_save_load_save_like_cpp(effects) {
        return false;
    }
    if effects.with_manager(|receiving_manager, effects| {
        live_popup_save_load_window_visible("PopupSaveLoad.wnd:SaveDescParent", receiving_manager)
    }) {
        return crate::gui::dispatch_os_click_named_window(
            "PopupSaveLoad.wnd:ButtonSaveDescConfirm",
            effects,
        );
    }
    if effects.with_manager(|receiving_manager, effects| {
        live_popup_save_load_window_visible(
            "PopupSaveLoad.wnd:OverwriteConfirmParent",
            receiving_manager,
        )
    }) {
        return crate::gui::dispatch_os_click_named_window(
            "PopupSaveLoad.wnd:ButtonOverwriteConfirm",
            effects,
        );
    }
    false
}

/// OS click on live `PopupSaveLoad.wnd:ButtonLoad` (not `simulate_*`).
pub fn drive_os_wnd_popup_save_load_load_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let _ = prepare_live_popup_save_load_for_click(effects);
    crate::gui::dispatch_os_click_named_window("PopupSaveLoad.wnd:ButtonLoad", effects)
}

/// Complete the retail load interaction: re-open, select a real save row,
/// click Load, then accept LoadConfirm. It intentionally returns false when
/// no real save exists rather than enabling ButtonLoad or inventing a row.
pub fn drive_os_wnd_popup_save_load_load_and_confirm_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if !select_first_live_popup_save_game_like_cpp(effects)
        || !crate::gui::dispatch_os_click_named_window("PopupSaveLoad.wnd:ButtonLoad", effects)
    {
        return false;
    }
    if !effects.with_manager(|receiving_manager, effects| {
        live_popup_save_load_window_visible(
            "PopupSaveLoad.wnd:LoadConfirmParent",
            receiving_manager,
        )
    }) {
        return false;
    }
    crate::gui::dispatch_os_click_named_window("PopupSaveLoad.wnd:ButtonLoadConfirm", effects)
}

/// Complete the retail load interaction for one exact filename: re-open,
/// select that genuine row, click Load, and accept LoadConfirm.
pub fn drive_os_wnd_popup_save_load_load_named_and_confirm_like_cpp(
    filename: &str,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if !select_live_popup_save_game_named_like_cpp(filename, effects)
        || !crate::gui::dispatch_os_click_named_window("PopupSaveLoad.wnd:ButtonLoad", effects)
    {
        return false;
    }
    if !effects.with_manager(|receiving_manager, effects| {
        live_popup_save_load_window_visible(
            "PopupSaveLoad.wnd:LoadConfirmParent",
            receiving_manager,
        )
    }) {
        return false;
    }
    crate::gui::dispatch_os_click_named_window("PopupSaveLoad.wnd:ButtonLoadConfirm", effects)
}

/// Residual: full-screen LoadOnly path: bind + select slot + Load.
pub fn simulate_save_load_menu_prepare_load(
    slot_index: i32,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if !simulate_save_load_menu_bind_layout(false, SaveLoadLayoutType::LoadOnly, effects) {
        return false;
    }
    if !simulate_save_load_menu_select_slot(slot_index, effects) {
        return false;
    }
    simulate_save_load_menu_load_button_gadget_selected(effects)
}

/// Focus delivery belongs to this callback's original tree and completes after
/// the selected-window loan is released, before the next admitted operation.
fn queue_save_load_focus(
    window: Rc<RefCell<GameWindow>>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    let window = Rc::downgrade(&window);
    effects.queue_window_operation(move |manager, effects| {
        let Some(window) = window.upgrade() else {
            return;
        };
        let id = window.borrow().get_id();
        let Some(current) = manager.get_window_by_id(id) else {
            return;
        };
        if Rc::ptr_eq(&current, &window) {
            let _ = manager.set_focus(Some(&window), effects);
        }
    });
}
