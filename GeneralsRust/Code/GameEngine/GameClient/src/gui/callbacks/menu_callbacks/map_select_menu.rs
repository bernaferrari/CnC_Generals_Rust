use super::*;

/// Map Select Menu implementation
pub struct MapSelectMenu {
    pub(super) residual_action: ResidualMapSelectMenuAction,

    pub(super) initialized: bool,
    pub(super) parent_id: i32,
    pub(super) listbox_map_id: i32,
    pub(super) button_ok_id: i32,
    pub(super) button_back_id: i32,
    pub(super) button_single_player_id: i32,
    pub(super) button_multiplayer_id: i32,
    pub(super) radio_easy_id: i32,
    pub(super) radio_medium_id: i32,
    pub(super) radio_hard_id: i32,
    pub(super) radio_system_maps_id: i32,
    pub(super) radio_user_maps_id: i32,
    pub(super) parent: Option<Rc<RefCell<GameWindow>>>,
    pub(super) listbox_map: Option<Rc<RefCell<GameWindow>>>,
    pub(super) show_solo_maps: bool,
    pub(super) use_system_maps: bool,
    pub(super) selected_map: Option<String>,
    pub(super) button_pushed: bool,
    pub(super) is_shutting_down: bool,
    pub(super) start_game: bool,
    pub(super) difficulty: i32,
}

impl MapSelectMenu {
    pub fn new() -> Self {
        Self {
            residual_action: ResidualMapSelectMenuAction::None,

            initialized: false,
            parent_id: 0,
            listbox_map_id: 0,
            button_ok_id: 0,
            button_back_id: 0,
            button_single_player_id: 0,
            button_multiplayer_id: 0,
            radio_easy_id: 0,
            radio_medium_id: 0,
            radio_hard_id: 0,
            radio_system_maps_id: 0,
            radio_user_maps_id: 0,
            parent: None,
            listbox_map: None,
            show_solo_maps: true,
            use_system_maps: true,
            selected_map: None,
            button_pushed: false,
            is_shutting_down: false,
            start_game: false,
            // C++ MapSelectMenu.cpp:35 `s_AIDiff = DIFFICULTY_NORMAL`; the script
            // engine is only consulted at MapSelectMenuInit (SetDifficultyRadioButton).
            difficulty: 1,
        }
    }

    pub(super) fn name_to_id(name: &str) -> i32 {
        NameKeyGenerator::name_to_key(name) as i32
    }

    pub(super) fn populate_map_list(&mut self) {
        let Some(listbox) = self.listbox_map.as_ref() else {
            return;
        };
        let mut listbox_guard = listbox.borrow_mut();
        let Some(widget) = listbox_guard.list_box_mut() else {
            return;
        };
        let map_to_select = self.selected_map.as_deref();
        populate_map_listbox(
            widget,
            self.use_system_maps,
            !self.show_solo_maps,
            map_to_select,
        );
        self.selected_map = widget
            .selected_item()
            .and_then(|item| match item.data.as_ref() {
                Some(ListBoxItemData::Text(path)) => Some(path.clone()),
                _ => None,
            });
    }

    pub(super) fn set_radio_selected(window: &Option<Rc<RefCell<GameWindow>>>, selected: bool) {
        let Some(window) = window.as_ref() else {
            return;
        };
        let mut guard = window.borrow_mut();
        if let Some(widget) = guard.widget_mut() {
            if let WindowWidget::RadioButton(radio) = widget {
                if selected {
                    radio.select();
                }
            }
        }
    }

    pub(super) fn update_selected_map(&mut self) {
        let Some(listbox) = self.listbox_map.as_ref() else {
            return;
        };
        let listbox_guard = listbox.borrow();
        let Some(widget) = listbox_guard.widget().and_then(|widget| match widget {
            WindowWidget::ListBox(listbox) => Some(listbox),
            _ => None,
        }) else {
            return;
        };
        self.selected_map = widget
            .selected_item()
            .and_then(|item| match item.data.as_ref() {
                Some(ListBoxItemData::Text(path)) => Some(path.clone()),
                _ => None,
            });
    }

    pub(super) fn set_map_selection_from_cpp_row(&mut self, row: i32) {
        let Some(listbox) = self.listbox_map.as_ref() else {
            return;
        };
        let mut listbox_guard = listbox.borrow_mut();
        let Some(widget) = listbox_guard.widget_mut() else {
            return;
        };
        let WindowWidget::ListBox(listbox) = widget else {
            return;
        };
        if row < 0 {
            listbox.set_selected_indices(&[]);
        } else {
            let _ = listbox.select_index(row as usize, crate::gui::gadgets::KeyModifiers::none());
        }
    }

    pub(super) fn start_game(
        &mut self,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        let Some(map_name) = self.selected_map.clone() else {
            return;
        };
        self.start_game = true;
        // Menus retain the authored INI/GameData Arc, while Main's AuthorityOnly
        // MSG_NEW_GAME drain reads the runtime GlobalData singleton.  Keep the
        // chosen map in both residences before the reverse animation can emit
        // the NewGame request (same ownership bridge as SkirmishGameOptions).
        {
            let pending = map_name;
            if let Some(data) = game_engine::common::ini::get_global_data() {
                let mut data = data.write();
                data.pending_file = pending.clone();
            }
            runtime_global_data::write().pending_file = pending;
        }
        queue_shell_reverse_animate_window(effects);
    }

    pub(super) fn do_game_start(&mut self, effects: &mut WindowEventEffects<'_>) {
        if TheGameLogic::is_in_game() {
            let _ = TheGameLogic::clear_game_data();
        }

        self.start_game = false;
        let mut msg = GameMessage::new(GameMessageType::NewGame);
        msg.append_integer_argument(GAME_SINGLE_PLAYER);
        msg.append_integer_argument(self.difficulty);
        msg.append_integer_argument(0);
        effects.emitted_messages.push(msg);
        init_random_with_seed(0);
        self.is_shutting_down = true;
    }

    pub(super) fn shutdown_complete(
        &mut self,
        layout: &WindowLayout,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        self.is_shutting_down = false;
        layout.hide_receiving(true, effects);
        self.initialized = false;
        self.parent = None;
        self.listbox_map = None;
        queue_shell_shutdown_complete(effects, false);
    }

    pub(super) fn write_use_system_maps_preference(&self) {
        let mut pref = UserPreferences::new();
        let _ = pref.load("Options.ini");
        pref.set_bool("UseSystemMapDir", self.use_system_maps);
        let _ = pref.write();
    }

    pub(super) fn update_map_cache(&self) {
        if let Ok(mut cache) = get_map_cache_manager().lock() {
            cache.update_cache();
        }
    }

    pub(super) fn refresh_map_list(&mut self) {
        self.update_map_cache();
        self.populate_map_list();
        self.update_selected_map();
    }

    pub(super) fn current_map_selection(&self) -> Option<String> {
        let Some(map_name) = self.selected_map.clone() else {
            return None;
        };
        Some(map_name)
    }
}

impl Default for MapSelectMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuCallbacks for MapSelectMenu {
    fn init(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Err("receiving menu lifecycle effects are required".into())
    }

    fn init_with_effects(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Initializing Map Select Menu for layout: {}",
            layout.get_filename()
        );
        self.initialized = true;
        self.button_pushed = false;
        self.show_solo_maps = true;
        self.is_shutting_down = false;
        self.start_game = false;
        show_shell_map_if_available(true, effects);
        layout.hide_receiving(false, effects);

        self.parent_id = Self::name_to_id("MapSelectMenu.wnd:MapSelectMenuParent");
        self.listbox_map_id = Self::name_to_id("MapSelectMenu.wnd:ListboxMap");
        self.button_ok_id = Self::name_to_id("MapSelectMenu.wnd:ButtonOK");
        self.button_back_id = Self::name_to_id("MapSelectMenu.wnd:ButtonBack");
        self.button_single_player_id = Self::name_to_id("MapSelectMenu.wnd:ButtonSinglePlayer");
        self.button_multiplayer_id = Self::name_to_id("MapSelectMenu.wnd:ButtonMultiplayer");
        self.radio_easy_id = Self::name_to_id("MapSelectMenu.wnd:RadioButtonEasyAI");
        self.radio_medium_id = Self::name_to_id("MapSelectMenu.wnd:RadioButtonMediumAI");
        self.radio_hard_id = Self::name_to_id("MapSelectMenu.wnd:RadioButtonHardAI");
        self.radio_user_maps_id = Self::name_to_id("MapSelectMenu.wnd:RadioButtonUserMaps");
        self.radio_system_maps_id = Self::name_to_id("MapSelectMenu.wnd:RadioButtonSystemMaps");

        effects.with_manager(|manager, effects| {
            self.parent = manager.get_window_by_id(self.parent_id);
            self.listbox_map = manager.get_window_by_id(self.listbox_map_id);
            // Focus is deferred (see SinglePlayerMenu::init): an inline
            // set_focus would re-enter the menu borrow held by this init
            // dispatch via the window's system callback.
            if let Some(parent) = self.parent.clone() {
                effects.queue_window_operation(move |manager, effects| {
                    let _ = manager.set_focus(Some(&parent), effects);
                });
            }
            if let Some(button_back) = manager.get_window_by_id(self.button_back_id) {
                effects.with_receiving_manager(manager, |effects| {
                    queue_shell_window_animation(
                        effects,
                        button_back,
                        AnimationType::SlideRight,
                        true,
                        0,
                    )
                });
            }
            if let Some(button_ok) = manager.get_window_by_id(self.button_ok_id) {
                effects.with_receiving_manager(manager, |effects| {
                    queue_shell_window_animation(
                        effects,
                        button_ok,
                        AnimationType::SlideLeft,
                        true,
                        0,
                    )
                });
            }
        });

        let mut pref = UserPreferences::new();
        let _ = pref.load("Options.ini");
        self.use_system_maps = pref.get_bool_or("UseSystemMapDir", true);

        let difficulty = gamelogic::helpers::TheScriptEngine::get_global_difficulty();
        self.difficulty = difficulty;

        Self::set_radio_selected(
            &effects.with_manager(|manager, effects| manager.get_window_by_id(self.radio_easy_id)),
            difficulty == 0,
        );
        Self::set_radio_selected(
            &effects
                .with_manager(|manager, effects| manager.get_window_by_id(self.radio_medium_id)),
            difficulty == 1,
        );
        Self::set_radio_selected(
            &effects.with_manager(|manager, effects| manager.get_window_by_id(self.radio_hard_id)),
            difficulty == 2,
        );
        Self::set_radio_selected(
            &effects.with_manager(|manager, effects| {
                manager.get_window_by_id(self.radio_system_maps_id)
            }),
            self.use_system_maps,
        );
        Self::set_radio_selected(
            &effects
                .with_manager(|manager, effects| manager.get_window_by_id(self.radio_user_maps_id)),
            !self.use_system_maps,
        );

        self.refresh_map_list();
        Ok(())
    }

    fn update(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.is_shutting_down && crate::gui::shell::base::shell_anim_finished_for_layout(effects)
        {
            self.shutdown_complete(layout, effects);
        }
        Ok(())
    }

    fn update_with_effects(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.start_game && crate::gui::shell::base::shell_anim_finished_for_layout(effects) {
            self.do_game_start(effects);
        }
        if self.is_shutting_down && crate::gui::shell::base::shell_anim_finished_for_layout(effects)
        {
            self.shutdown_complete(layout, effects);
        }
        Ok(())
    }

    fn shutdown(
        &mut self,
        layout: &WindowLayout,
        user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Shutting down Map Select Menu for layout: {}",
            layout.get_filename()
        );
        let pop_immediate = user_data
            .and_then(|data| data.downcast_ref::<bool>())
            .copied()
            .unwrap_or(false);
        if pop_immediate {
            self.shutdown_complete(layout, effects);
            return Ok(());
        }
        if !self.start_game {
            self.is_shutting_down = true;
            queue_shell_reverse_animate_window(effects);
        }
        Ok(())
    }

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
            WindowMessage::GadgetSelected => {
                if self.button_pushed {
                    return WindowMsgHandled::Handled;
                }

                let control_id = data1 as i32;
                if control_id == self.button_ok_id {
                    self.update_selected_map();
                    if self.current_map_selection().is_some() {
                        self.button_pushed = true;
                        if let Some(manager) = effects.campaign_manager.as_deref_mut() {
                            manager.set_campaign("");
                        } else {
                            return WindowMsgHandled::Handled;
                        }
                        self.start_game(effects);
                        return WindowMsgHandled::Handled;
                    }
                    return WindowMsgHandled::Handled;
                }
                if control_id == self.button_back_id {
                    self.button_pushed = true;
                    queue_shell_pop(effects);
                    return WindowMsgHandled::Handled;
                }
                if control_id == self.button_single_player_id {
                    self.show_solo_maps = true;
                    self.refresh_map_list();
                    return WindowMsgHandled::Handled;
                }
                if control_id == self.button_multiplayer_id {
                    self.show_solo_maps = false;
                    self.refresh_map_list();
                    return WindowMsgHandled::Handled;
                }
                if control_id == self.radio_system_maps_id {
                    self.use_system_maps = true;
                    self.write_use_system_maps_preference();
                    self.refresh_map_list();
                    return WindowMsgHandled::Handled;
                }
                if control_id == self.radio_user_maps_id {
                    self.use_system_maps = false;
                    self.write_use_system_maps_preference();
                    self.refresh_map_list();
                    return WindowMsgHandled::Handled;
                }
                if control_id == self.radio_easy_id {
                    self.difficulty = 0;
                    return WindowMsgHandled::Handled;
                }
                if control_id == self.radio_medium_id {
                    self.difficulty = 1;
                    return WindowMsgHandled::Handled;
                }
                if control_id == self.radio_hard_id {
                    self.difficulty = 2;
                    return WindowMsgHandled::Handled;
                }
                if control_id == self.listbox_map_id {
                    self.update_selected_map();
                    return WindowMsgHandled::Handled;
                }
                WindowMsgHandled::Ignored
            }
            WindowMessage::GadgetValueChanged => {
                let control_id = data1 as i32;
                if control_id == self.listbox_map_id {
                    self.update_selected_map();
                    return WindowMsgHandled::Handled;
                }
                WindowMsgHandled::Ignored
            }
            WindowMessage::User(code) if code == GLM_DOUBLE_CLICKED => {
                if self.button_pushed {
                    return WindowMsgHandled::Handled;
                }
                let control_id = data1 as i32;
                if control_id == self.listbox_map_id {
                    self.set_map_selection_from_cpp_row(data2 as i32);
                    self.update_selected_map();
                    if self.current_map_selection().is_some() {
                        if let Some(parent) = self.parent.as_ref() {
                            effects.queue_system_message(
                                Rc::downgrade(parent),
                                WindowMessage::GadgetSelected,
                                self.button_ok_id as WindowMsgData,
                                self.button_ok_id as WindowMsgData,
                            );
                        }
                    }
                    return WindowMsgHandled::Handled;
                }
                WindowMsgHandled::Ignored
            }
            _ => WindowMsgHandled::Ignored,
        }
    }

    fn input(
        &mut self,
        _window: &mut GameWindow,
        _msg: WindowMessage,
        _data1: WindowMsgData,
        _data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        panic!("menu input dispatch requires receiving event effects");
    }

    fn input_with_effects(
        &mut self,
        _window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        if msg != WindowMessage::Char || self.button_pushed {
            return WindowMsgHandled::Ignored;
        }
        let key = data1 as u32;
        let state = data2 as u32;
        if key != 0x1B {
            return WindowMsgHandled::Ignored;
        }
        if (state & 0x0001) == 0 {
            return WindowMsgHandled::Handled;
        }
        if let Some(parent) = self.parent.clone() {
            effects.queue_system_message(
                Rc::downgrade(&parent),
                WindowMessage::GadgetSelected,
                self.button_back_id as WindowMsgData,
                self.button_back_id as WindowMsgData,
            );
        }
        WindowMsgHandled::Handled
    }
}

/// Credits Menu implementation

/// Residual: last MapSelectMenu action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualMapSelectMenuAction {
    None = 0,
    SelectMap = 1,
    Ok = 2,
    Back = 3,
    SoloMaps = 4,
    MultiplayerMaps = 5,
    SystemMaps = 6,
    UserMaps = 7,
    DifficultyEasy = 8,
    DifficultyMedium = 9,
    DifficultyHard = 10,
}

pub fn residual_map_select_menu_last_action(
    effects: &mut WindowEventEffects<'_>,
) -> ResidualMapSelectMenuAction {
    effects.with_map_select_menu(|menu, _effects| menu.residual_action)
}

pub fn residual_map_select_menu_difficulty(effects: &mut WindowEventEffects<'_>) -> i32 {
    effects.with_map_select_menu(|menu, _effects| menu.difficulty)
}

pub fn residual_map_select_menu_selected_map(
    effects: &mut WindowEventEffects<'_>,
) -> Option<String> {
    effects.with_map_select_menu(|menu, _effects| menu.selected_map.clone())
}

pub(super) fn ensure_map_select_control_ids(menu: &mut MapSelectMenu) {
    if menu.parent_id == 0 {
        menu.parent_id =
            NameKeyGenerator::name_to_key("MapSelectMenu.wnd:MapSelectMenuParent") as i32;
    }
    if menu.listbox_map_id == 0 {
        menu.listbox_map_id = NameKeyGenerator::name_to_key("MapSelectMenu.wnd:ListboxMap") as i32;
    }
    if menu.button_ok_id == 0 {
        menu.button_ok_id = NameKeyGenerator::name_to_key("MapSelectMenu.wnd:ButtonOK") as i32;
    }
    if menu.button_back_id == 0 {
        menu.button_back_id = NameKeyGenerator::name_to_key("MapSelectMenu.wnd:ButtonBack") as i32;
    }
    if menu.button_single_player_id == 0 {
        menu.button_single_player_id =
            NameKeyGenerator::name_to_key("MapSelectMenu.wnd:ButtonSinglePlayer") as i32;
    }
    if menu.button_multiplayer_id == 0 {
        menu.button_multiplayer_id =
            NameKeyGenerator::name_to_key("MapSelectMenu.wnd:ButtonMultiplayer") as i32;
    }
    if menu.radio_easy_id == 0 {
        menu.radio_easy_id =
            NameKeyGenerator::name_to_key("MapSelectMenu.wnd:RadioButtonEasyAI") as i32;
    }
    if menu.radio_medium_id == 0 {
        menu.radio_medium_id =
            NameKeyGenerator::name_to_key("MapSelectMenu.wnd:RadioButtonMediumAI") as i32;
    }
    if menu.radio_hard_id == 0 {
        menu.radio_hard_id =
            NameKeyGenerator::name_to_key("MapSelectMenu.wnd:RadioButtonHardAI") as i32;
    }
    if menu.radio_system_maps_id == 0 {
        menu.radio_system_maps_id =
            NameKeyGenerator::name_to_key("MapSelectMenu.wnd:RadioButtonSystemMaps") as i32;
    }
    if menu.radio_user_maps_id == 0 {
        menu.radio_user_maps_id =
            NameKeyGenerator::name_to_key("MapSelectMenu.wnd:RadioButtonUserMaps") as i32;
    }
}

/// Residual: bind MapSelectMenu control IDs (no layout/list populate).
pub fn simulate_map_select_menu_bind_controls(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_map_select_menu(|menu, effects| {
        ensure_map_select_control_ids(menu);
        menu.initialized = true;
        menu.button_pushed = false;
        menu.is_shutting_down = false;
        menu.start_game = false;
        true
    })
}

/// Residual: select a map path without live listbox.
pub fn simulate_map_select_menu_select_map(
    map_path: &str,
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    if map_path.is_empty() {
        return false;
    }
    effects.with_map_select_menu(|menu, effects| {
        ensure_map_select_control_ids(menu);
        menu.selected_map = Some(map_path.to_string());

        menu.residual_action = ResidualMapSelectMenuAction::SelectMap;
        menu.selected_map.clone().as_deref() == Some(map_path)
    })
}

/// Residual: set AI difficulty radio (0/1/2) without widgets.
pub fn simulate_map_select_menu_set_difficulty(
    difficulty: i32,
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    if !(0..=2).contains(&difficulty) {
        return false;
    }
    effects.with_map_select_menu(|menu, effects| {
        ensure_map_select_control_ids(menu);
        menu.difficulty = difficulty;

        menu.residual_action = match difficulty {
            0 => ResidualMapSelectMenuAction::DifficultyEasy,
            2 => ResidualMapSelectMenuAction::DifficultyHard,
            _ => ResidualMapSelectMenuAction::DifficultyMedium,
        };
        menu.difficulty == difficulty
    })
}

/// Residual: fire ButtonOK without start_game engine transfer.
pub fn simulate_map_select_menu_ok_button_gadget_selected(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    effects.with_map_select_menu(|menu, effects| {
        ensure_map_select_control_ids(menu);
        if menu.button_pushed {
            return false;
        }
        if menu.selected_map.is_none() {
            // C++ ignores OK with no selection.
            return false;
        }
        menu.button_pushed = true;
        // Do not call start_game(); residual latch only.
        menu.residual_action = ResidualMapSelectMenuAction::Ok;
        true
    })
}

/// Residual: fire ButtonBack without shell pop.
pub fn simulate_map_select_menu_back_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_map_select_menu(|menu, effects| {
        ensure_map_select_control_ids(menu);
        if menu.button_pushed {
            return false;
        }
        menu.button_pushed = true;
        menu.residual_action = ResidualMapSelectMenuAction::Back;
        true
    })
}

/// Residual: ButtonSinglePlayer map filter residual.
pub fn simulate_map_select_menu_solo_maps_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_map_select_menu(|menu, effects| {
        ensure_map_select_control_ids(menu);
        menu.show_solo_maps = true;
        menu.residual_action = ResidualMapSelectMenuAction::SoloMaps;
        true
    })
}

/// Residual: ButtonMultiplayer map filter residual.
pub fn simulate_map_select_menu_multiplayer_maps_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_map_select_menu(|menu, effects| {
        ensure_map_select_control_ids(menu);
        menu.show_solo_maps = false;
        menu.residual_action = ResidualMapSelectMenuAction::MultiplayerMaps;
        true
    })
}

/// Residual: RadioButtonSystemMaps residual.
pub fn simulate_map_select_menu_system_maps_radio_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_map_select_menu(|menu, effects| {
        ensure_map_select_control_ids(menu);
        menu.use_system_maps = true;
        menu.residual_action = ResidualMapSelectMenuAction::SystemMaps;
        true
    })
}

/// Residual: RadioButtonUserMaps residual.
pub fn simulate_map_select_menu_user_maps_radio_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_map_select_menu(|menu, effects| {
        ensure_map_select_control_ids(menu);
        menu.use_system_maps = false;
        menu.residual_action = ResidualMapSelectMenuAction::UserMaps;
        true
    })
}

/// Residual: clear button_pushed latch.
pub fn simulate_map_select_menu_clear_button_pushed(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_map_select_menu(|menu, effects| {
        menu.button_pushed = false;
        menu.start_game = false;
        true
    })
}

/// Residual: select map + medium AI + OK composite (campaign start honesty).
pub fn simulate_map_select_menu_prepare_ok(
    map_path: &str,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if !simulate_map_select_menu_bind_controls(effects) {
        return false;
    }
    let _ = simulate_map_select_menu_clear_button_pushed(effects);
    if !simulate_map_select_menu_select_map(map_path, effects) {
        return false;
    }
    if !simulate_map_select_menu_set_difficulty(1, effects) {
        return false;
    }
    simulate_map_select_menu_ok_button_gadget_selected(effects)
}
