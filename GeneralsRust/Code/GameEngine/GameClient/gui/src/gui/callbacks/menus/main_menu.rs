use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/MainMenu.cpp",
    "crate::gui::callbacks::menus::main_menu",
    "Main Menu",
    "Primary shell landing screen.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "MainMenu",
    "Main Menu",
    "Front-door shell menu for starting or configuring the game.",
    "Shell",
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MainMenuDropdownPort {
    None,
    Single,
    Multiplayer,
    LoadReplay,
    Difficulty,
}

impl MainMenuDropdownPort {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "Default",
            Self::Single => "Single Player",
            Self::Multiplayer => "Multiplayer",
            Self::LoadReplay => "Load / Replay",
            Self::Difficulty => "Difficulty",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CampaignSidePort {
    Training,
    Usa,
    Gla,
    China,
    Skirmish,
}

impl CampaignSidePort {
    pub fn label(self) -> &'static str {
        match self {
            Self::Training => "Challenge",
            Self::Usa => "USA",
            Self::Gla => "GLA",
            Self::China => "China",
            Self::Skirmish => "Skirmish",
        }
    }

    pub fn default_map(self) -> &'static str {
        match self {
            Self::Training => "ChallengeLadder",
            Self::Usa => "USA01",
            Self::Gla => "GLA01",
            Self::China => "China01",
            Self::Skirmish => "TournamentDesert",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MainMenuAction {
    PushScreen {
        window: String,
        reverse: &'static str,
        show_dropdown: Option<MainMenuDropdownPort>,
    },
    Transition {
        remove: &'static str,
        reverse: &'static str,
        set_group: &'static str,
        show_dropdown: Option<MainMenuDropdownPort>,
    },
    ShowOptions,
    StartGame {
        fade: &'static str,
    },
    QuitImmediate,
    ShowQuitDialog,
    LaunchWorldBuilder,
    StartPatchCheck,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum MainMenuButtonId {
    SinglePlayer,
    MultiPlayer,
    Skirmish,
    Online,
    Network,
    Options,
    Credits,
    Replay,
    LoadReplay,
    LoadGame,
    Challenge,
    Usa,
    Gla,
    China,
    Easy,
    Medium,
    Hard,
    DiffBack,
    MultiBack,
    SingleBack,
    LoadReplayBack,
    Exit,
    WorldBuilder,
    GetUpdate,
}

impl MainMenuButtonId {
    pub const WND_PREFIX: &'static str = "MainMenu.wnd:";

    pub fn from_name_key(key: &str) -> Option<Self> {
        let name = key.strip_prefix(Self::WND_PREFIX)?;
        let button = name.strip_prefix("Button")?;
        match button {
            "SinglePlayer" => Some(Self::SinglePlayer),
            "Multiplayer" => Some(Self::MultiPlayer),
            "Skirmish" => Some(Self::Skirmish),
            "Online" => Some(Self::Online),
            "Network" => Some(Self::Network),
            "Options" => Some(Self::Options),
            "Credits" => Some(Self::Credits),
            "Replay" => Some(Self::Replay),
            "LoadReplay" => Some(Self::LoadReplay),
            "LoadGame" => Some(Self::LoadGame),
            "Challenge" => Some(Self::Challenge),
            "USA" => Some(Self::Usa),
            "GLA" => Some(Self::Gla),
            "China" => Some(Self::China),
            "Easy" => Some(Self::Easy),
            "Medium" => Some(Self::Medium),
            "Hard" => Some(Self::Hard),
            "DiffBack" => Some(Self::DiffBack),
            "MultiBack" => Some(Self::MultiBack),
            "SingleBack" => Some(Self::SingleBack),
            "LoadReplayBack" => Some(Self::LoadReplayBack),
            "Exit" => Some(Self::Exit),
            "WorldBuilder" => Some(Self::WorldBuilder),
            "GetUpdate" => Some(Self::GetUpdate),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameDifficultyPort {
    Easy,
    Normal,
    Hard,
}

impl GameDifficultyPort {
    pub fn label(self) -> &'static str {
        match self {
            Self::Easy => "Easy",
            Self::Normal => "Medium",
            Self::Hard => "Hard",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingGameStartPort {
    pub map_name: String,
    pub difficulty: GameDifficultyPort,
    pub opens_challenge_menu: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SelectiveButtonsPort {
    pub usa_recent_save: bool,
    pub usa_load_game: bool,
    pub gla_recent_save: bool,
    pub gla_load_game: bool,
    pub china_recent_save: bool,
    pub china_load_game: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MainMenuPort {
    pub shell_map_visible: bool,
    pub button_pushed: bool,
    pub is_shutting_down: bool,
    pub start_game: bool,
    pub initial_gadget_delay: u16,
    pub drop_down: MainMenuDropdownPort,
    pub pending_drop_down: MainMenuDropdownPort,
    pub campaign_selected: bool,
    pub dont_allow_transitions: bool,
    pub raise_message_boxes: bool,
    pub show_logo: bool,
    pub show_frames: u16,
    pub show_side: Option<CampaignSidePort>,
    pub logo_is_shown: bool,
    pub just_entered: bool,
    pub launch_challenge_menu: bool,
    pub selected_campaign: Option<CampaignSidePort>,
    pub selective_buttons: SelectiveButtonsPort,
    pub pending_game_start: Option<PendingGameStartPort>,
    pub last_shell_push: Option<String>,
    pub options_menu_visible: bool,
    pub quit_requested: bool,
}

impl Default for MainMenuPort {
    fn default() -> Self {
        Self::init()
    }
}

impl MainMenuPort {
    pub fn init() -> Self {
        Self {
            shell_map_visible: true,
            button_pushed: false,
            is_shutting_down: false,
            start_game: false,
            initial_gadget_delay: 2,
            drop_down: MainMenuDropdownPort::None,
            pending_drop_down: MainMenuDropdownPort::None,
            campaign_selected: false,
            dont_allow_transitions: false,
            raise_message_boxes: true,
            show_logo: false,
            show_frames: 0,
            show_side: None,
            logo_is_shown: false,
            just_entered: true,
            launch_challenge_menu: false,
            selected_campaign: None,
            selective_buttons: SelectiveButtonsPort::default(),
            pending_game_start: None,
            last_shell_push: None,
            options_menu_visible: false,
            quit_requested: false,
        }
    }

    pub fn update(&mut self, transitions_finished: bool, shell_anim_finished: bool) -> bool {
        if self.just_entered {
            if self.initial_gadget_delay == 1 {
                self.initial_gadget_delay = 2;
                self.just_entered = false;
            } else {
                self.initial_gadget_delay = self.initial_gadget_delay.saturating_sub(1);
            }
        }

        if self.dont_allow_transitions && transitions_finished {
            self.dont_allow_transitions = false;
        }

        if self.start_game && transitions_finished && shell_anim_finished {
            self.start_game = false;
            self.is_shutting_down = true;
            return true;
        }

        false
    }

    pub fn enter_single_player(&mut self) -> bool {
        if self.dont_allow_transitions {
            return false;
        }
        self.dont_allow_transitions = true;
        self.button_pushed = false;
        self.drop_down = MainMenuDropdownPort::Single;
        self.pending_drop_down = MainMenuDropdownPort::None;
        true
    }

    pub fn back_from_single_player(&mut self) -> bool {
        if self.campaign_selected || self.dont_allow_transitions {
            return false;
        }
        self.dont_allow_transitions = true;
        self.button_pushed = false;
        self.drop_down = MainMenuDropdownPort::None;
        true
    }

    pub fn enter_multiplayer(&mut self) -> bool {
        if self.dont_allow_transitions {
            return false;
        }
        self.dont_allow_transitions = true;
        self.button_pushed = false;
        self.drop_down = MainMenuDropdownPort::Multiplayer;
        true
    }

    pub fn back_from_multiplayer(&mut self) -> bool {
        if self.dont_allow_transitions {
            return false;
        }
        self.dont_allow_transitions = true;
        self.button_pushed = false;
        self.drop_down = MainMenuDropdownPort::None;
        true
    }

    pub fn enter_load_replay(&mut self) -> bool {
        if self.dont_allow_transitions {
            return false;
        }
        self.dont_allow_transitions = true;
        self.button_pushed = false;
        self.drop_down = MainMenuDropdownPort::LoadReplay;
        true
    }

    pub fn back_from_load_replay(&mut self) -> bool {
        if self.dont_allow_transitions {
            return false;
        }
        self.dont_allow_transitions = true;
        self.button_pushed = false;
        self.drop_down = MainMenuDropdownPort::None;
        true
    }

    pub fn open_credits(&mut self) -> bool {
        if self.dont_allow_transitions {
            return false;
        }
        self.dont_allow_transitions = true;
        self.button_pushed = true;
        self.last_shell_push = Some("Menus/CreditsMenu.wnd".to_string());
        true
    }

    pub fn open_options(&mut self) -> bool {
        if self.dont_allow_transitions {
            return false;
        }
        self.dont_allow_transitions = true;
        self.options_menu_visible = true;
        true
    }

    pub fn open_skirmish(&mut self) -> bool {
        if self.campaign_selected || self.dont_allow_transitions {
            return false;
        }
        self.button_pushed = true;
        self.campaign_selected = true;
        self.selected_campaign = Some(CampaignSidePort::Skirmish);
        self.show_side = Some(CampaignSidePort::Skirmish);
        self.last_shell_push = Some("Menus/SkirmishGameOptionsMenu.wnd".to_string());
        true
    }

    pub fn open_network_lobby(&mut self) -> bool {
        if self.dont_allow_transitions {
            return false;
        }
        self.dont_allow_transitions = true;
        self.button_pushed = true;
        self.last_shell_push = Some("Menus/LanLobbyMenu.wnd".to_string());
        true
    }

    pub fn open_replay_menu(&mut self) -> bool {
        if self.dont_allow_transitions {
            return false;
        }
        self.dont_allow_transitions = true;
        self.button_pushed = true;
        self.last_shell_push = Some("Menus/ReplayMenu.wnd".to_string());
        true
    }

    pub fn open_save_load(&mut self) -> bool {
        if self.dont_allow_transitions {
            return false;
        }
        self.dont_allow_transitions = true;
        self.button_pushed = true;
        self.last_shell_push = Some("Menus/SaveLoad.wnd".to_string());
        true
    }

    pub fn select_challenge(&mut self) -> bool {
        if self.campaign_selected || self.dont_allow_transitions {
            return false;
        }
        self.campaign_selected = true;
        self.drop_down = MainMenuDropdownPort::Difficulty;
        self.show_side = Some(CampaignSidePort::Training);
        self.launch_challenge_menu = true;
        self.show_logo = false;
        self.selected_campaign = Some(CampaignSidePort::Training);
        true
    }

    pub fn select_campaign_side(&mut self, side: CampaignSidePort) -> bool {
        if self.campaign_selected || self.dont_allow_transitions {
            return false;
        }
        self.campaign_selected = true;
        self.selected_campaign = Some(side);
        self.drop_down = MainMenuDropdownPort::Difficulty;
        self.show_side = Some(side);
        self.show_logo = false;
        self.launch_challenge_menu = false;
        self.show_selective_buttons(side);
        true
    }

    pub fn select_difficulty(&mut self, difficulty: GameDifficultyPort) -> bool {
        if self.dont_allow_transitions {
            return false;
        }
        let Some(side) = self.selected_campaign else {
            return false;
        };

        self.button_pushed = false;
        self.pending_game_start = Some(PendingGameStartPort {
            map_name: side.default_map().to_string(),
            difficulty,
            opens_challenge_menu: self.launch_challenge_menu,
        });

        if self.launch_challenge_menu {
            self.last_shell_push = Some("Menus/ChallengeMenu.wnd".to_string());
        } else {
            self.start_game = true;
        }

        true
    }

    pub fn back_from_difficulty(&mut self) -> bool {
        if self.dont_allow_transitions {
            return false;
        }
        self.dont_allow_transitions = true;
        self.campaign_selected = false;
        self.selected_campaign = None;
        self.show_side = None;
        self.drop_down = MainMenuDropdownPort::Single;
        self.launch_challenge_menu = false;
        self.selective_buttons = SelectiveButtonsPort::default();
        true
    }

    pub fn handle_button(
        &mut self,
        button: MainMenuButtonId,
        windowed: bool,
    ) -> Option<MainMenuAction> {
        if self.button_pushed
            && button != MainMenuButtonId::Easy
            && button != MainMenuButtonId::Medium
            && button != MainMenuButtonId::Hard
        {
            return None;
        }

        match button {
            MainMenuButtonId::SinglePlayer => {
                if self.enter_single_player() {
                    Some(MainMenuAction::Transition {
                        remove: "MainMenuDefaultMenu",
                        reverse: "MainMenuDefaultMenuBack",
                        set_group: "MainMenuSinglePlayerMenu",
                        show_dropdown: Some(MainMenuDropdownPort::Single),
                    })
                } else {
                    None
                }
            }
            MainMenuButtonId::SingleBack => {
                if self.back_from_single_player() {
                    Some(MainMenuAction::Transition {
                        remove: "MainMenuSinglePlayerMenu",
                        reverse: "MainMenuSinglePlayerMenuBack",
                        set_group: "MainMenuDefaultMenu",
                        show_dropdown: Some(MainMenuDropdownPort::None),
                    })
                } else {
                    None
                }
            }
            MainMenuButtonId::MultiPlayer => {
                if self.enter_multiplayer() {
                    Some(MainMenuAction::Transition {
                        remove: "MainMenuDefaultMenu",
                        reverse: "MainMenuDefaultMenuBack",
                        set_group: "MainMenuMultiPlayerMenu",
                        show_dropdown: Some(MainMenuDropdownPort::Multiplayer),
                    })
                } else {
                    None
                }
            }
            MainMenuButtonId::MultiBack => {
                if self.back_from_multiplayer() {
                    Some(MainMenuAction::Transition {
                        remove: "MainMenuMultiPlayerMenu",
                        reverse: "MainMenuMultiPlayerMenuReverse",
                        set_group: "MainMenuDefaultMenu",
                        show_dropdown: Some(MainMenuDropdownPort::None),
                    })
                } else {
                    None
                }
            }
            MainMenuButtonId::LoadReplay => {
                if self.enter_load_replay() {
                    Some(MainMenuAction::Transition {
                        remove: "MainMenuDefaultMenu",
                        reverse: "MainMenuDefaultMenuBack",
                        set_group: "MainMenuLoadReplayMenu",
                        show_dropdown: Some(MainMenuDropdownPort::LoadReplay),
                    })
                } else {
                    None
                }
            }
            MainMenuButtonId::LoadReplayBack => {
                if self.back_from_load_replay() {
                    Some(MainMenuAction::Transition {
                        remove: "MainMenuLoadReplayMenu",
                        reverse: "MainMenuLoadReplayMenuBack",
                        set_group: "MainMenuDefaultMenu",
                        show_dropdown: Some(MainMenuDropdownPort::None),
                    })
                } else {
                    None
                }
            }
            MainMenuButtonId::Credits => {
                if self.open_credits() {
                    Some(MainMenuAction::PushScreen {
                        window: "Menus/CreditsMenu.wnd".to_string(),
                        reverse: "MainMenuDefaultMenu",
                        show_dropdown: Some(MainMenuDropdownPort::None),
                    })
                } else {
                    None
                }
            }
            MainMenuButtonId::Skirmish => {
                if self.open_skirmish() {
                    Some(MainMenuAction::PushScreen {
                        window: "Menus/SkirmishGameOptionsMenu.wnd".to_string(),
                        reverse: "MainMenuSinglePlayerMenuBackSkirmish",
                        show_dropdown: Some(MainMenuDropdownPort::Single),
                    })
                } else {
                    None
                }
            }
            MainMenuButtonId::Online => {
                if self.dont_allow_transitions {
                    return None;
                }
                self.dont_allow_transitions = true;
                self.button_pushed = true;
                Some(MainMenuAction::PushScreen {
                    window: "Menus/WOLLoginMenu.wnd".to_string(),
                    reverse: "MainMenuMultiPlayerMenuTransitionToNext",
                    show_dropdown: Some(MainMenuDropdownPort::Multiplayer),
                })
            }
            MainMenuButtonId::Network => {
                if self.open_network_lobby() {
                    Some(MainMenuAction::PushScreen {
                        window: "Menus/LanLobbyMenu.wnd".to_string(),
                        reverse: "MainMenuMultiPlayerMenuTransitionToNext",
                        show_dropdown: Some(MainMenuDropdownPort::Multiplayer),
                    })
                } else {
                    None
                }
            }
            MainMenuButtonId::Options => {
                if self.open_options() {
                    Some(MainMenuAction::ShowOptions)
                } else {
                    None
                }
            }
            MainMenuButtonId::Replay => {
                if self.open_replay_menu() {
                    Some(MainMenuAction::PushScreen {
                        window: "Menus/ReplayMenu.wnd".to_string(),
                        reverse: "MainMenuLoadReplayMenuBackTransition",
                        show_dropdown: Some(MainMenuDropdownPort::LoadReplay),
                    })
                } else {
                    None
                }
            }
            MainMenuButtonId::LoadGame => {
                if self.open_save_load() {
                    Some(MainMenuAction::PushScreen {
                        window: "Menus/SaveLoad.wnd".to_string(),
                        reverse: "MainMenuLoadReplayMenuBackTransition",
                        show_dropdown: Some(MainMenuDropdownPort::LoadReplay),
                    })
                } else {
                    None
                }
            }
            MainMenuButtonId::Challenge => {
                if self.select_challenge() {
                    Some(MainMenuAction::Transition {
                        remove: "",
                        reverse: "MainMenuSinglePlayerMenuBackTraining",
                        set_group: "MainMenuDifficultyMenuTraining",
                        show_dropdown: Some(MainMenuDropdownPort::Difficulty),
                    })
                } else {
                    None
                }
            }
            MainMenuButtonId::Usa => {
                if self.select_campaign_side(CampaignSidePort::Usa) {
                    Some(MainMenuAction::Transition {
                        remove: "",
                        reverse: "MainMenuSinglePlayerMenuBackUS",
                        set_group: "MainMenuDifficultyMenuUS",
                        show_dropdown: Some(MainMenuDropdownPort::Difficulty),
                    })
                } else {
                    None
                }
            }
            MainMenuButtonId::Gla => {
                if self.select_campaign_side(CampaignSidePort::Gla) {
                    Some(MainMenuAction::Transition {
                        remove: "",
                        reverse: "MainMenuSinglePlayerMenuBackGLA",
                        set_group: "MainMenuDifficultyMenuGLA",
                        show_dropdown: Some(MainMenuDropdownPort::Difficulty),
                    })
                } else {
                    None
                }
            }
            MainMenuButtonId::China => {
                if self.select_campaign_side(CampaignSidePort::China) {
                    Some(MainMenuAction::Transition {
                        remove: "",
                        reverse: "MainMenuSinglePlayerMenuBackChina",
                        set_group: "MainMenuDifficultyMenuChina",
                        show_dropdown: Some(MainMenuDropdownPort::Difficulty),
                    })
                } else {
                    None
                }
            }
            MainMenuButtonId::Easy => {
                if self.select_difficulty(GameDifficultyPort::Easy) {
                    Some(self.difficulty_action())
                } else {
                    None
                }
            }
            MainMenuButtonId::Medium => {
                if self.select_difficulty(GameDifficultyPort::Normal) {
                    Some(self.difficulty_action())
                } else {
                    None
                }
            }
            MainMenuButtonId::Hard => {
                if self.select_difficulty(GameDifficultyPort::Hard) {
                    Some(self.difficulty_action())
                } else {
                    None
                }
            }
            MainMenuButtonId::DiffBack => {
                if self.back_from_difficulty() {
                    let reverse = match self.show_side {
                        Some(CampaignSidePort::Training) => "MainMenuDifficultyMenuTrainingBack",
                        Some(CampaignSidePort::Usa) => "MainMenuDifficultyMenuUSBack",
                        Some(CampaignSidePort::Gla) => "MainMenuDifficultyMenuGLABack",
                        Some(CampaignSidePort::China) => "MainMenuDifficultyMenuChinaBack",
                        _ => "",
                    };
                    Some(MainMenuAction::Transition {
                        remove: "",
                        reverse,
                        set_group: "",
                        show_dropdown: Some(MainMenuDropdownPort::Single),
                    })
                } else {
                    None
                }
            }
            MainMenuButtonId::Exit => {
                self.quit(windowed);
                if windowed {
                    Some(MainMenuAction::QuitImmediate)
                } else {
                    Some(MainMenuAction::ShowQuitDialog)
                }
            }
            MainMenuButtonId::WorldBuilder => Some(MainMenuAction::LaunchWorldBuilder),
            MainMenuButtonId::GetUpdate => Some(MainMenuAction::StartPatchCheck),
        }
    }

    fn difficulty_action(&self) -> MainMenuAction {
        if self.launch_challenge_menu {
            MainMenuAction::PushScreen {
                window: "Menus/ChallengeMenu.wnd".to_string(),
                reverse: "MainMenuDifficultyMenuTraining",
                show_dropdown: None,
            }
        } else {
            MainMenuAction::StartGame {
                fade: "FadeWholeScreen",
            }
        }
    }

    pub fn quit(&mut self, windowed: bool) {
        self.button_pushed = true;
        self.quit_requested = true;
        if windowed {
            self.is_shutting_down = true;
        }
    }

    fn show_selective_buttons(&mut self, side: CampaignSidePort) {
        self.selective_buttons = match side {
            CampaignSidePort::Usa => SelectiveButtonsPort {
                usa_recent_save: true,
                usa_load_game: true,
                ..SelectiveButtonsPort::default()
            },
            CampaignSidePort::Gla => SelectiveButtonsPort {
                gla_recent_save: true,
                gla_load_game: true,
                ..SelectiveButtonsPort::default()
            },
            CampaignSidePort::China => SelectiveButtonsPort {
                china_recent_save: true,
                china_load_game: true,
                ..SelectiveButtonsPort::default()
            },
            CampaignSidePort::Training | CampaignSidePort::Skirmish => {
                SelectiveButtonsPort::default()
            }
        };
    }

    pub fn sample() -> Self {
        let mut state = Self::init();
        let _ = state.update(false, false);
        let _ = state.update(false, false);
        let _ = state.enter_single_player();
        state.dont_allow_transitions = false;
        let _ = state.select_campaign_side(CampaignSidePort::Usa);
        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selecting_campaign_side_opens_difficulty_and_side_specific_buttons() {
        let mut menu = MainMenuPort::init();
        let _ = menu.enter_single_player();
        menu.dont_allow_transitions = false;

        assert!(menu.select_campaign_side(CampaignSidePort::China));
        assert_eq!(menu.drop_down, MainMenuDropdownPort::Difficulty);
        assert!(menu.campaign_selected);
        assert!(menu.selective_buttons.china_recent_save);
        assert!(menu.selective_buttons.china_load_game);
        assert!(!menu.selective_buttons.usa_recent_save);
    }

    #[test]
    fn challenge_selection_launches_challenge_menu_after_difficulty_pick() {
        let mut menu = MainMenuPort::init();

        assert!(menu.select_challenge());
        assert!(menu.select_difficulty(GameDifficultyPort::Hard));
        assert_eq!(
            menu.last_shell_push.as_deref(),
            Some("Menus/ChallengeMenu.wnd")
        );
        assert_eq!(
            menu.pending_game_start,
            Some(PendingGameStartPort {
                map_name: "ChallengeLadder".to_string(),
                difficulty: GameDifficultyPort::Hard,
                opens_challenge_menu: true,
            })
        );
    }

    #[test]
    fn update_starts_game_when_transitions_and_shell_finish() {
        let mut menu = MainMenuPort::sample();

        assert!(menu.select_difficulty(GameDifficultyPort::Normal));
        assert!(menu.update(true, true));
        assert!(menu.is_shutting_down);
        assert!(!menu.start_game);
    }

    #[test]
    fn dispatch_single_player_pushes_transition() {
        let mut menu = MainMenuPort::init();
        let action = menu.handle_button(MainMenuButtonId::SinglePlayer, false);
        assert_eq!(
            action,
            Some(MainMenuAction::Transition {
                remove: "MainMenuDefaultMenu",
                reverse: "MainMenuDefaultMenuBack",
                set_group: "MainMenuSinglePlayerMenu",
                show_dropdown: Some(MainMenuDropdownPort::Single),
            })
        );
        assert!(menu.dont_allow_transitions);
    }

    #[test]
    fn dispatch_credits_pushes_credits_menu() {
        let mut menu = MainMenuPort::init();
        let action = menu.handle_button(MainMenuButtonId::Credits, false);
        assert_eq!(
            action,
            Some(MainMenuAction::PushScreen {
                window: "Menus/CreditsMenu.wnd".to_string(),
                reverse: "MainMenuDefaultMenu",
                show_dropdown: Some(MainMenuDropdownPort::None),
            })
        );
        assert_eq!(
            menu.last_shell_push.as_deref(),
            Some("Menus/CreditsMenu.wnd")
        );
    }

    #[test]
    fn dispatch_skirmish_pushes_skirmish_options() {
        let mut menu = MainMenuPort::init();
        let action = menu.handle_button(MainMenuButtonId::Skirmish, false);
        assert_eq!(
            action,
            Some(MainMenuAction::PushScreen {
                window: "Menus/SkirmishGameOptionsMenu.wnd".to_string(),
                reverse: "MainMenuSinglePlayerMenuBackSkirmish",
                show_dropdown: Some(MainMenuDropdownPort::Single),
            })
        );
        assert!(menu.campaign_selected);
    }

    #[test]
    fn dispatch_network_pushes_lan_lobby() {
        let mut menu = MainMenuPort::init();
        let action = menu.handle_button(MainMenuButtonId::Network, false);
        assert_eq!(
            action,
            Some(MainMenuAction::PushScreen {
                window: "Menus/LanLobbyMenu.wnd".to_string(),
                reverse: "MainMenuMultiPlayerMenuTransitionToNext",
                show_dropdown: Some(MainMenuDropdownPort::Multiplayer),
            })
        );
    }

    #[test]
    fn dispatch_online_pushes_wol_login() {
        let mut menu = MainMenuPort::init();
        let action = menu.handle_button(MainMenuButtonId::Online, false);
        assert_eq!(
            action,
            Some(MainMenuAction::PushScreen {
                window: "Menus/WOLLoginMenu.wnd".to_string(),
                reverse: "MainMenuMultiPlayerMenuTransitionToNext",
                show_dropdown: Some(MainMenuDropdownPort::Multiplayer),
            })
        );
    }

    #[test]
    fn dispatch_replay_pushes_replay_menu() {
        let mut menu = MainMenuPort::init();
        let action = menu.handle_button(MainMenuButtonId::Replay, false);
        assert_eq!(
            action,
            Some(MainMenuAction::PushScreen {
                window: "Menus/ReplayMenu.wnd".to_string(),
                reverse: "MainMenuLoadReplayMenuBackTransition",
                show_dropdown: Some(MainMenuDropdownPort::LoadReplay),
            })
        );
    }

    #[test]
    fn dispatch_exit_windowed_quits_immediately() {
        let mut menu = MainMenuPort::init();
        let action = menu.handle_button(MainMenuButtonId::Exit, true);
        assert_eq!(action, Some(MainMenuAction::QuitImmediate));
        assert!(menu.quit_requested);
        assert!(menu.is_shutting_down);
    }

    #[test]
    fn dispatch_exit_fullscreen_shows_dialog() {
        let mut menu = MainMenuPort::init();
        let action = menu.handle_button(MainMenuButtonId::Exit, false);
        assert_eq!(action, Some(MainMenuAction::ShowQuitDialog));
        assert!(menu.quit_requested);
        assert!(!menu.is_shutting_down);
    }

    #[test]
    fn dispatch_blocked_when_dont_allow_transitions() {
        let mut menu = MainMenuPort::init();
        menu.dont_allow_transitions = true;
        assert!(menu
            .handle_button(MainMenuButtonId::Credits, false)
            .is_none());
        assert!(menu
            .handle_button(MainMenuButtonId::Skirmish, false)
            .is_none());
        assert!(menu
            .handle_button(MainMenuButtonId::Network, false)
            .is_none());
        assert!(menu
            .handle_button(MainMenuButtonId::Options, false)
            .is_none());
    }

    #[test]
    fn dispatch_blocked_when_button_already_pushed() {
        let mut menu = MainMenuPort::init();
        menu.button_pushed = true;
        assert!(menu
            .handle_button(MainMenuButtonId::Credits, false)
            .is_none());
    }

    #[test]
    fn difficulty_buttons_bypass_button_pushed_guard() {
        let mut menu = MainMenuPort::init();
        let _ = menu.enter_single_player();
        menu.dont_allow_transitions = false;
        let _ = menu.select_campaign_side(CampaignSidePort::Usa);
        menu.button_pushed = true;
        assert!(menu.handle_button(MainMenuButtonId::Easy, false).is_some());
    }

    #[test]
    fn from_name_key_parses_button_ids() {
        assert_eq!(
            MainMenuButtonId::from_name_key("MainMenu.wnd:ButtonCredits"),
            Some(MainMenuButtonId::Credits)
        );
        assert_eq!(
            MainMenuButtonId::from_name_key("MainMenu.wnd:ButtonSkirmish"),
            Some(MainMenuButtonId::Skirmish)
        );
        assert_eq!(
            MainMenuButtonId::from_name_key("MainMenu.wnd:ButtonExit"),
            Some(MainMenuButtonId::Exit)
        );
        assert_eq!(
            MainMenuButtonId::from_name_key("Other.wnd:ButtonCredits"),
            None
        );
    }
}
