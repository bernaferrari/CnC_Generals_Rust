use gpui::{
    div, hsla, img, prelude::*, px, rgb, size, AnyElement, App, Application, Bounds, Context,
    Image, ImageFormat, Window, WindowBounds, WindowOptions,
};
use gpui_gui::{CampaignSidePort, GameDifficultyPort};
use log::{info, warn};
use std::fs;
use std::io::Cursor;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

pub fn run() -> anyhow::Result<()> {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1600.0), px(980.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| EditorApp::new()),
        )
        .expect("failed to open experimental editor window");
        cx.activate(true);
    });
    Ok(())
}

struct EditorApp {
    runtime: RuntimeBridge,
    menu_note: Option<String>,
    menu_logo_image: Option<Arc<Image>>,
    menu_ruler_image: Option<Arc<Image>>,
    menu_backdrop_image: Option<Arc<Image>>,
    loading_background_image: Option<Arc<Image>>,
    selected_challenge_general: usize,
    selected_challenge_difficulty: Option<GameDifficultyPort>,
    last_menu_screen: String,
    menu_input_cooldown_until: Option<Instant>,
}

#[derive(Clone, Copy)]
enum MenuButtonAction {
    SinglePlayer,
    Multiplayer,
    LoadReplay,
    Options,
    Credits,
    MessageOfDay,
    GetUpdates,
    WorldBuilder,
    ExitGame,
    Back,
    Challenge,
    UsaCampaign,
    GlaCampaign,
    ChinaCampaign,
    Skirmish,
    Online,
    Network,
    LoadGame,
    LoadLatestSave,
    Replay,
    ReplayLatest,
    StartSkirmishNow,
    DifficultyEasy,
    DifficultyMedium,
    DifficultyHard,
    ChallengeGeneral(usize),
    ChallengeStart,
    OnlineQuickMatch,
    OnlineCustomMatch,
    NetworkHost,
    NetworkJoin,
    TogglePause,
    MainMenu,
}

#[derive(Clone, Copy)]
struct ChallengeGeneralEntry {
    id: &'static str,
    label: &'static str,
}

const CHALLENGE_GENERALS: [ChallengeGeneralEntry; 9] = [
    ChallengeGeneralEntry {
        id: "townes",
        label: "General Townes",
    },
    ChallengeGeneralEntry {
        id: "kwai",
        label: "General Kwai",
    },
    ChallengeGeneralEntry {
        id: "alexander",
        label: "General Alexander",
    },
    ChallengeGeneralEntry {
        id: "tao",
        label: "General Tao",
    },
    ChallengeGeneralEntry {
        id: "thrax",
        label: "Dr. Thrax",
    },
    ChallengeGeneralEntry {
        id: "fai",
        label: "General Fai",
    },
    ChallengeGeneralEntry {
        id: "juhziz",
        label: "Prince Kassad",
    },
    ChallengeGeneralEntry {
        id: "demolitions",
        label: "General Demo",
    },
    ChallengeGeneralEntry {
        id: "boss",
        label: "General Leang",
    },
];

impl EditorApp {
    fn new() -> Self {
        Self {
            runtime: RuntimeBridge::new(),
            menu_note: None,
            menu_logo_image: Self::load_menu_logo_image(),
            menu_ruler_image: Self::load_menu_ruler_image(),
            menu_backdrop_image: Self::load_menu_backdrop_image(),
            loading_background_image: Self::load_loading_background_image(),
            selected_challenge_general: 0,
            selected_challenge_difficulty: None,
            last_menu_screen: "MainMenu".to_string(),
            menu_input_cooldown_until: None,
        }
    }

    fn sync_menu_screen(&mut self) {
        let screen = self.current_menu_screen().to_string();
        if screen != self.last_menu_screen {
            self.last_menu_screen = screen;
            self.menu_input_cooldown_until = Some(Instant::now() + Duration::from_millis(120));
            self.menu_note = None;
            if self.current_menu_screen() != "Challenge" {
                self.selected_challenge_general = 0;
            }
        }
    }

    fn can_accept_menu_input(&self) -> bool {
        self.menu_input_cooldown_until
            .map(|deadline| Instant::now() >= deadline)
            .unwrap_or(true)
    }

    fn mark_menu_input_handled(&mut self) {
        self.menu_input_cooldown_until = Some(Instant::now() + Duration::from_millis(80));
    }

    fn send_runtime_command(&mut self, command: &str, success_note: String) {
        match self.runtime.send_command(command) {
            Ok(()) => {
                self.menu_note = Some(success_note);
            }
            Err(err) => {
                self.menu_note = Some(format!("Runtime command '{command}' failed: {err}"));
            }
        }
    }

    fn difficulty_token(difficulty: GameDifficultyPort) -> &'static str {
        match difficulty {
            GameDifficultyPort::Easy => "easy",
            GameDifficultyPort::Normal => "medium",
            GameDifficultyPort::Hard => "hard",
        }
    }

    fn build_start_game_command(
        mode: &'static str,
        faction: &'static str,
        map: &str,
        difficulty: Option<GameDifficultyPort>,
    ) -> String {
        let difficulty_token = difficulty
            .map(|d: GameDifficultyPort| d.label().to_ascii_lowercase())
            .unwrap_or_else(|| "normal".to_string());
        format!(
            "start_game|mode={mode}|faction={faction}|map={}|difficulty={difficulty_token}",
            map.trim()
        )
    }

    fn start_selected_difficulty(&mut self, difficulty: GameDifficultyPort) {
        let campaign = self.current_difficulty_campaign();
        if campaign == Some(CampaignSidePort::Training) {
            self.selected_challenge_difficulty = Some(difficulty);
            let command = format!(
                "open_challenge_menu|difficulty={}",
                Self::difficulty_token(difficulty)
            );
            self.send_runtime_command(
                &command,
                format!("Opening challenge menu ({})", difficulty.label()),
            );
            return;
        }

        let (faction, map) = match campaign.unwrap_or(CampaignSidePort::Usa) {
            CampaignSidePort::Usa => ("USA", CampaignSidePort::Usa.default_map()),
            CampaignSidePort::Gla => ("GLA", CampaignSidePort::Gla.default_map()),
            CampaignSidePort::China => ("China", CampaignSidePort::China.default_map()),
            CampaignSidePort::Skirmish => ("USA", CampaignSidePort::Skirmish.default_map()),
            CampaignSidePort::Training => ("USA", CampaignSidePort::Training.default_map()),
        };
        let command =
            Self::build_start_game_command("singleplayer", faction, map, Some(difficulty));
        self.send_runtime_command(
            &command,
            format!("Starting campaign {} ({})", map, difficulty.label()),
        );
    }

    fn current_menu_screen(&self) -> &str {
        self.runtime.ui_screen_name().unwrap_or("MainMenu")
    }

    fn current_difficulty_campaign(&self) -> Option<CampaignSidePort> {
        match self.current_menu_screen() {
            "DifficultyChallenge" => Some(CampaignSidePort::Training),
            "DifficultyUsa" => Some(CampaignSidePort::Usa),
            "DifficultyGla" => Some(CampaignSidePort::Gla),
            "DifficultyChina" => Some(CampaignSidePort::China),
            _ => None,
        }
    }

    fn handle_menu_button(&mut self, action: MenuButtonAction, cx: &mut Context<Self>) {
        if !self.can_accept_menu_input() {
            cx.notify();
            return;
        }
        self.mark_menu_input_handled();

        match action {
            MenuButtonAction::ExitGame => {
                self.send_runtime_command("exit", "Requested runtime shutdown.".to_string());
                cx.quit();
            }
            MenuButtonAction::SinglePlayer => {
                self.send_runtime_command(
                    "open_single_player_menu",
                    "Opened single player menu.".to_string(),
                );
            }
            MenuButtonAction::Multiplayer => {
                self.send_runtime_command(
                    "open_multiplayer_menu",
                    "Opened multiplayer menu.".to_string(),
                );
            }
            MenuButtonAction::LoadReplay => {
                self.send_runtime_command(
                    "open_load_replay_menu",
                    "Opened load/replay menu.".to_string(),
                );
            }
            MenuButtonAction::Options => {
                self.send_runtime_command("open_options", "Opening options menu.".to_string());
            }
            MenuButtonAction::Credits => {
                self.send_runtime_command("open_credits", "Opening credits.".to_string());
            }
            MenuButtonAction::MessageOfDay => {
                self.send_runtime_command(
                    "open_message_of_the_day",
                    "Opening message of the day.".to_string(),
                );
            }
            MenuButtonAction::GetUpdates => {
                self.send_runtime_command("open_get_updates", "Opening updates.".to_string());
            }
            MenuButtonAction::WorldBuilder => {
                self.send_runtime_command("open_world_builder", "Launching World Builder.".to_string());
            }
            MenuButtonAction::Back => match self.current_menu_screen() {
                "DifficultyChallenge" | "DifficultyUsa" | "DifficultyGla" | "DifficultyChina" => {
                    self.send_runtime_command(
                        "open_single_player_menu",
                        "Returning to single player menu.".to_string(),
                    );
                }
                "SinglePlayer" | "Multiplayer" | "LoadReplay" | "Options" | "Credits"
                | "Skirmish" | "LoadGame" | "Online" | "Network" | "Replay" | "Challenge"
                | "MessageOfDay" | "GetUpdates" | "WorldBuilder" => {
                    self.send_runtime_command("menu", "Returning to main menu.".to_string());
                }
                _ => {
                    if !self.runtime.on_main_menu_screen() {
                        self.send_runtime_command("menu", "Returning to main menu.".to_string());
                    }
                }
            },
            MenuButtonAction::Challenge => {
                self.send_runtime_command(
                    "open_difficulty_menu|campaign=challenge",
                    "Opening challenge difficulty options.".to_string(),
                );
            }
            MenuButtonAction::UsaCampaign => {
                self.send_runtime_command(
                    "open_difficulty_menu|campaign=usa",
                    "Opening USA campaign difficulty.".to_string(),
                );
            }
            MenuButtonAction::GlaCampaign => {
                self.send_runtime_command(
                    "open_difficulty_menu|campaign=gla",
                    "Opening GLA campaign difficulty.".to_string(),
                );
            }
            MenuButtonAction::ChinaCampaign => {
                self.send_runtime_command(
                    "open_difficulty_menu|campaign=china",
                    "Opening China campaign difficulty.".to_string(),
                );
            }
            MenuButtonAction::Skirmish => {
                self.send_runtime_command(
                    "open_skirmish_menu",
                    "Opening skirmish options.".to_string(),
                );
            }
            MenuButtonAction::Online => {
                self.send_runtime_command("open_online", "Opening online services.".to_string());
            }
            MenuButtonAction::Network => {
                self.send_runtime_command("open_network", "Opening network lobby.".to_string());
            }
            MenuButtonAction::LoadGame => {
                self.send_runtime_command("open_load_game", "Opening load game.".to_string());
            }
            MenuButtonAction::LoadLatestSave => {
                if let Some(slot) = self.runtime.latest_save_slot() {
                    let command = format!("load_game|slot={slot}");
                    self.send_runtime_command(
                        &command,
                        format!("Loading most recent save '{slot}'."),
                    );
                } else {
                    self.menu_note =
                        Some("No save files found in runtime Save Games directory.".to_string());
                }
            }
            MenuButtonAction::Replay => {
                self.send_runtime_command("open_replay", "Opening replay menu.".to_string());
            }
            MenuButtonAction::ReplayLatest => {
                if let Some(slot) = self.runtime.latest_replay_slot() {
                    let command = format!("replay|slot={slot}");
                    self.send_runtime_command(&command, format!("Starting replay '{slot}'."));
                } else {
                    self.menu_note = Some(
                        "No replay files found in runtime Save Games/Replays directory."
                            .to_string(),
                    );
                }
            }
            MenuButtonAction::StartSkirmishNow => {
                let map = CampaignSidePort::Skirmish.default_map();
                let command = Self::build_start_game_command("skirmish", "USA", map, None);
                self.send_runtime_command(&command, format!("Starting skirmish on {map}."));
            }
            MenuButtonAction::DifficultyEasy => {
                self.start_selected_difficulty(GameDifficultyPort::Easy)
            }
            MenuButtonAction::DifficultyMedium => {
                self.start_selected_difficulty(GameDifficultyPort::Normal)
            }
            MenuButtonAction::DifficultyHard => {
                self.start_selected_difficulty(GameDifficultyPort::Hard)
            }
            MenuButtonAction::ChallengeGeneral(index) => {
                if index < CHALLENGE_GENERALS.len() {
                    self.selected_challenge_general = index;
                    self.menu_note =
                        Some(format!("Selected {}", CHALLENGE_GENERALS[index].label));
                }
            }
            MenuButtonAction::ChallengeStart => {
                let Some(difficulty) = self.selected_challenge_difficulty else {
                    self.menu_note = Some(
                        "Pick challenge difficulty first (Easy/Medium/Hard).".to_string(),
                    );
                    cx.notify();
                    return;
                };
                let general = CHALLENGE_GENERALS[self.selected_challenge_general];
                let command = format!(
                    "start_game|mode=challenge|faction=USA|map=ChallengeLadder|difficulty={}|opponent={}",
                    Self::difficulty_token(difficulty),
                    general.id
                );
                self.send_runtime_command(
                    &command,
                    format!(
                        "Starting Generals Challenge vs {} ({})",
                        general.label,
                        difficulty.label()
                    ),
                );
            }
            MenuButtonAction::OnlineQuickMatch => {
                let command = Self::build_start_game_command(
                    "internet",
                    "USA",
                    CampaignSidePort::Skirmish.default_map(),
                    None,
                );
                self.send_runtime_command(&command, "Starting online quick match.".to_string());
            }
            MenuButtonAction::OnlineCustomMatch => {
                self.send_runtime_command(
                    "open_online",
                    "Opening online custom match browser.".to_string(),
                );
            }
            MenuButtonAction::NetworkHost => {
                let command = Self::build_start_game_command(
                    "network",
                    "USA",
                    CampaignSidePort::Skirmish.default_map(),
                    None,
                );
                self.send_runtime_command(&command, "Hosting LAN match.".to_string());
            }
            MenuButtonAction::NetworkJoin => {
                self.send_runtime_command("open_network", "Opening LAN join panel.".to_string());
            }
            MenuButtonAction::TogglePause => {
                self.send_runtime_command("toggle_pause", "Toggled pause.".to_string());
            }
            MenuButtonAction::MainMenu => {
                self.send_runtime_command("menu", "Returning to main menu.".to_string());
            }
        }

        cx.notify();
    }

    fn menu_panel_title(&self) -> &'static str {
        match self.current_menu_screen() {
            "SinglePlayer" => "Single Player",
            "Multiplayer" => "Multiplayer",
            "LoadReplay" => "Load / Replay",
            "DifficultyChallenge" | "DifficultyUsa" | "DifficultyGla" | "DifficultyChina" => {
                "Select Difficulty"
            }
            "Options" => "Options",
            "Credits" => "Credits",
            "LoadGame" => "Load Game",
            "SaveGame" => "Save Game",
            "Skirmish" => "Skirmish",
            "Online" => "Online",
            "Network" => "Network",
            "Replay" => "Replay",
            "Challenge" => "Generals Challenge",
            "MessageOfDay" => "Message of the Day",
            "GetUpdates" => "Get Updates",
            "WorldBuilder" => "World Builder",
            _ => "Main Menu",
        }
    }

    fn menu_buttons(&self) -> Vec<(MenuButtonAction, &'static str)> {
        if self.runtime.state() == "Playing" {
            return vec![
                (MenuButtonAction::TogglePause, "Pause / Resume"),
                (MenuButtonAction::MainMenu, "Main Menu"),
                (MenuButtonAction::ExitGame, "Exit"),
            ];
        }

        match self.current_menu_screen() {
            "MainMenu" => vec![
                (MenuButtonAction::SinglePlayer, "Single Player"),
                (MenuButtonAction::Multiplayer, "Multiplayer"),
                (MenuButtonAction::LoadReplay, "Load / Replay"),
                (MenuButtonAction::Options, "Options"),
                (MenuButtonAction::Credits, "Credits"),
                (MenuButtonAction::MessageOfDay, "Message of the Day"),
                (MenuButtonAction::GetUpdates, "Get Updates"),
                (MenuButtonAction::WorldBuilder, "World Builder"),
                (MenuButtonAction::ExitGame, "Exit"),
            ],
            "SinglePlayer" => vec![
                (MenuButtonAction::UsaCampaign, "USA Campaign"),
                (MenuButtonAction::GlaCampaign, "GLA Campaign"),
                (MenuButtonAction::ChinaCampaign, "China Campaign"),
                (MenuButtonAction::Challenge, "Challenge"),
                (MenuButtonAction::Skirmish, "Skirmish"),
                (MenuButtonAction::Back, "Back"),
            ],
            "Multiplayer" => vec![
                (MenuButtonAction::Online, "Online"),
                (MenuButtonAction::Network, "Network"),
                (MenuButtonAction::Back, "Back"),
            ],
            "LoadReplay" => vec![
                (MenuButtonAction::LoadGame, "Load Game"),
                (MenuButtonAction::Replay, "Replay"),
                (MenuButtonAction::Back, "Back"),
            ],
            "DifficultyUsa" | "DifficultyGla" | "DifficultyChina" | "DifficultyChallenge" => {
                vec![
                    (MenuButtonAction::DifficultyEasy, "Easy"),
                    (MenuButtonAction::DifficultyMedium, "Medium"),
                    (MenuButtonAction::DifficultyHard, "Hard"),
                    (MenuButtonAction::Back, "Back"),
                ]
            }
            "Skirmish" => vec![
                (MenuButtonAction::StartSkirmishNow, "Start Skirmish"),
                (MenuButtonAction::Back, "Back"),
            ],
            "Options" | "Credits" => vec![(MenuButtonAction::Back, "Back")],
            "LoadGame" => vec![
                (MenuButtonAction::LoadLatestSave, "Load Latest Save"),
                (MenuButtonAction::Back, "Back"),
            ],
            "Online" => vec![
                (MenuButtonAction::OnlineQuickMatch, "Quick Match"),
                (MenuButtonAction::OnlineCustomMatch, "Custom Match"),
                (MenuButtonAction::Back, "Back"),
            ],
            "Network" => vec![
                (MenuButtonAction::NetworkHost, "Host"),
                (MenuButtonAction::NetworkJoin, "Join"),
                (MenuButtonAction::Back, "Back"),
            ],
            "Replay" => vec![
                (MenuButtonAction::ReplayLatest, "Play Latest Replay"),
                (MenuButtonAction::Back, "Back"),
            ],
            "Challenge" => {
                let mut buttons = Vec::with_capacity(CHALLENGE_GENERALS.len() + 2);
                for (index, general) in CHALLENGE_GENERALS.iter().enumerate() {
                    buttons.push((MenuButtonAction::ChallengeGeneral(index), general.label));
                }
                buttons.push((MenuButtonAction::ChallengeStart, "Start Challenge"));
                buttons.push((MenuButtonAction::Back, "Back"));
                buttons
            }
            "MessageOfDay" | "GetUpdates" | "WorldBuilder" => {
                vec![(MenuButtonAction::Back, "Back")]
            }
            _ => vec![
                (MenuButtonAction::Back, "Back"),
                (MenuButtonAction::ExitGame, "Exit"),
            ],
        }
    }

    fn render_overlay_button(
        &self,
        id: &'static str,
        label: &'static str,
        action: MenuButtonAction,
        danger: bool,
        button_width: f32,
        button_height: f32,
        text_padding_left: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (border, background, text_color) = if danger {
            (rgb(0xf3c7c7), hsla(0.98, 0.55, 0.31, 0.94), rgb(0xfff6f6))
        } else {
            (rgb(0xe2e8ff), hsla(0.62, 0.62, 0.34, 0.95), rgb(0xffffff))
        };
        div()
            .id(id)
            .w(px(button_width))
            .h(px(button_height))
            .flex()
            .items_center()
            .justify_start()
            .border_1()
            .border_color(border)
            .bg(background)
            .cursor_pointer()
            .pl(px(text_padding_left))
            .text_color(text_color)
            .text_base()
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| this.handle_menu_button(action, cx)))
            .into_any_element()
    }

    fn load_image_candidates(candidates: &[&str]) -> Option<Arc<Image>> {
        candidates
            .iter()
            .find_map(|candidate| Self::load_ui_image(Path::new(candidate)))
    }

    fn load_ui_image(path: &Path) -> Option<Arc<Image>> {
        if !path.is_file() {
            return None;
        }
        let bytes = fs::read(path).ok()?;
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .unwrap_or_default();

        let try_direct = match extension.as_str() {
            "png" => Some(ImageFormat::Png),
            "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
            "gif" => Some(ImageFormat::Gif),
            "webp" => Some(ImageFormat::Webp),
            _ => None,
        };
        if let Some(format) = try_direct {
            return Some(Arc::new(Image::from_bytes(format, bytes)));
        }

        let decoded = image::load_from_memory(&bytes).ok()?;
        let mut png_bytes = Vec::new();
        decoded
            .write_to(&mut Cursor::new(&mut png_bytes), image::ImageFormat::Png)
            .ok()?;
        Some(Arc::new(Image::from_bytes(ImageFormat::Png, png_bytes)))
    }

    fn load_menu_logo_image() -> Option<Arc<Image>> {
        let candidates = [
            "windows_game/extracted_big_files/TexturesZH/Art/Textures/sclogosuserinterface512_001.tga",
            "windows_game/extracted_big_files_v2/TexturesZH/Art/Textures/sclogosuserinterface512_001.tga",
            "windows_game/extracted_big_files/EnglishZH/Data/English/Art/Textures/TitleScreenuserinterface.tga",
            "windows_game/extracted_big_files_v2/EnglishZH/Data/English/Art/Textures/TitleScreenuserinterface.tga",
        ];
        Self::load_image_candidates(&candidates)
    }

    fn load_menu_ruler_image() -> Option<Arc<Image>> {
        let candidates = [
            "windows_game/extracted_big_files/TexturesZH/Art/Textures/mainmenuruleruserinterface.tga",
            "windows_game/extracted_big_files_v2/TexturesZH/Art/Textures/mainmenuruleruserinterface.tga",
        ];
        Self::load_image_candidates(&candidates)
    }

    fn load_menu_backdrop_image() -> Option<Arc<Image>> {
        let candidates = [
            "windows_game/extracted_big_files/EnglishZH/Data/English/Art/Textures/TitleScreenuserinterface.tga",
            "windows_game/extracted_big_files/EnglishZH/Data/English/Art/Textures/loadpageuserinterface.tga",
            "windows_game/extracted_big_files_v2/EnglishZH/Data/English/Art/Textures/TitleScreenuserinterface.tga",
            "windows_game/extracted_big_files_v2/EnglishZH/Data/English/Art/Textures/loadpageuserinterface.tga",
        ];
        Self::load_image_candidates(&candidates)
    }

    fn load_loading_background_image() -> Option<Arc<Image>> {
        let candidates = [
            "windows_game/extracted_big_files/EnglishZH/Data/English/Art/Textures/loadpageuserinterface.tga",
            "windows_game/extracted_big_files/EnglishZH/Data/English/Art/Textures/Skirmish_Loaduserinterface.tga",
            "windows_game/extracted_big_files/TexturesZH/Art/Textures/mp_loaduserinterface_00b.tga",
            "windows_game/extracted_big_files_v2/EnglishZH/Data/English/Art/Textures/loadpageuserinterface.tga",
            "windows_game/extracted_big_files_v2/EnglishZH/Data/English/Art/Textures/Skirmish_Loaduserinterface.tga",
            "windows_game/extracted_big_files_v2/TexturesZH/Art/Textures/mp_loaduserinterface_00b.tga",
        ];
        Self::load_image_candidates(&candidates)
    }

    fn runtime_overlay(
        &self,
        viewport_width: f32,
        viewport_height: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let runtime_state = self.runtime.state();
        let is_loading = matches!(runtime_state, "Booting" | "Loading");
        if is_loading {
            let scale_x = (viewport_width / 800.0).max(0.25);
            let progress = self.runtime.startup_progress();
            let track_width = (296.0 * scale_x).max(180.0);
            let track_height = (16.0 * scale_x.clamp(0.6, 1.25)).max(10.0);
            let progress_width = (track_width * progress).clamp(0.0, track_width);
            return div()
                .id("viewport-loading-overlay")
                .absolute()
                .top_0()
                .left_0()
                .w_full()
                .h_full()
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .w_full()
                        .h_full()
                        .when_some(self.loading_background_image.clone(), |layer, image| {
                            layer.child(img(image).w_full().h_full().max_w_full())
                        })
                        .when(self.loading_background_image.is_none(), |layer| {
                            layer.bg(rgb(0x0a1118))
                        }),
                )
                .child(
                    div()
                        .absolute()
                        .bottom(px(28.0))
                        .left_0()
                        .w_full()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .w(px(track_width))
                                .h(px(track_height))
                                .rounded_md()
                                .border_1()
                                .border_color(rgb(0x7d6a45))
                                .bg(hsla(0.62, 0.35, 0.10, 0.86))
                                .child(
                                    div()
                                        .h_full()
                                        .w(px(progress_width))
                                        .rounded_md()
                                        .bg(rgb(0xd2b36a)),
                                ),
                        )
                        .child(div().text_sm().text_color(rgb(0xf5e9d7)).child(format!(
                            "{} {:.0}%",
                            self.runtime.startup_phase(),
                            progress * 100.0
                        )))
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(0xc9d6e2))
                                .child(self.runtime.status_line()),
                        ),
                )
                .into_any_element();
        }

        let canvas_scale = (viewport_width / 800.0).min(viewport_height / 600.0).max(0.55);
        let canvas_width = 800.0 * canvas_scale;
        let canvas_height = 600.0 * canvas_scale;
        let canvas_left = ((viewport_width - canvas_width) * 0.5).max(0.0);
        let canvas_top = ((viewport_height - canvas_height) * 0.5).max(0.0);
        let show_menu_overlay = !(runtime_state == "Playing" && self.current_menu_screen() == "GameHUD");
        if !show_menu_overlay {
            return div()
                .id("viewport-menu-overlay-empty")
                .absolute()
                .top_0()
                .left_0()
                .w_full()
                .h_full()
                .into_any_element();
        }

        let logo_left = canvas_left + 504.0 * canvas_scale;
        let logo_top = canvas_top + 16.0 * canvas_scale;
        let logo_width = (791.0 - 504.0) * canvas_scale;
        let logo_height = (110.0 - 16.0) * canvas_scale;

        let ruler_left = canvas_left + 532.0 * canvas_scale;
        let ruler_top = canvas_top + 102.0 * canvas_scale;
        let ruler_width = (756.0 - 532.0) * canvas_scale;
        let ruler_height = (116.0 - 102.0) * canvas_scale;

        let panel_left = canvas_left + 532.0 * canvas_scale;
        let panel_top = canvas_top + 108.0 * canvas_scale;
        let panel_width = (756.0 - 532.0) * canvas_scale;
        let panel_padding = (8.0 * canvas_scale).max(6.0);
        let button_width = (panel_width - panel_padding * 2.0).max(140.0);
        let button_height = (34.0 * canvas_scale.clamp(0.7, 1.3)).max(24.0);
        let text_padding_left = (18.0 * canvas_scale).max(10.0);

        div()
            .id("viewport-menu-overlay")
            .absolute()
            .top_0()
            .left_0()
            .w_full()
            .h_full()
            .child(
                div()
                    .absolute()
                    .top(px(canvas_top))
                    .left(px(canvas_left))
                    .w(px(canvas_width))
                    .h(px(canvas_height))
                    .when_some(self.menu_backdrop_image.clone(), |layer, image| {
                        layer.child(img(image).w_full().h_full().max_w_full())
                    })
                    .when(self.menu_backdrop_image.is_none(), |layer| {
                        layer.bg(hsla(0.60, 0.30, 0.10, 0.52))
                    })
                    .child(div().absolute().top_0().left_0().w_full().h_full().bg(hsla(
                        0.60, 0.22, 0.08, 0.22,
                    ))),
            )
            .when_some(self.menu_ruler_image.clone(), |overlay, ruler| {
                overlay.child(
                    div()
                        .absolute()
                        .top(px(ruler_top))
                        .left(px(ruler_left))
                        .w(px(ruler_width))
                        .h(px(ruler_height.max(6.0)))
                        .child(img(ruler).w_full().h_full().max_w_full()),
                )
            })
            .when_some(self.menu_logo_image.clone(), |overlay, logo| {
                overlay.child(
                    div()
                        .absolute()
                        .top(px(logo_top))
                        .left(px(logo_left))
                        .w(px(logo_width))
                        .h(px(logo_height))
                        .child(img(logo).w_full().h_full().max_w_full()),
                )
            })
            .child(
                div()
                    .absolute()
                    .top(px(panel_top))
                    .left(px(panel_left))
                    .w(px(panel_width))
                    .p(px(panel_padding))
                    .border_1()
                    .border_color(rgb(0x3a4f97))
                    .bg(hsla(0.61, 0.56, 0.20, 0.68))
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap_2()
                    .children(self.menu_buttons().into_iter().map(|(action, label)| {
                        let id = match action {
                            MenuButtonAction::SinglePlayer => "menu-single-player",
                            MenuButtonAction::Multiplayer => "menu-multiplayer",
                            MenuButtonAction::LoadReplay => "menu-load-replay",
                            MenuButtonAction::Options => "menu-options",
                            MenuButtonAction::Credits => "menu-credits",
                            MenuButtonAction::MessageOfDay => "menu-motd",
                            MenuButtonAction::GetUpdates => "menu-updates",
                            MenuButtonAction::WorldBuilder => "menu-world-builder",
                            MenuButtonAction::ExitGame => "menu-exit-game",
                            MenuButtonAction::Back => "menu-back",
                            MenuButtonAction::Challenge => "menu-challenge",
                            MenuButtonAction::UsaCampaign => "menu-usa",
                            MenuButtonAction::GlaCampaign => "menu-gla",
                            MenuButtonAction::ChinaCampaign => "menu-china",
                            MenuButtonAction::Skirmish => "menu-skirmish",
                            MenuButtonAction::Online => "menu-online",
                            MenuButtonAction::Network => "menu-network",
                            MenuButtonAction::LoadGame => "menu-load-game",
                            MenuButtonAction::LoadLatestSave => "menu-load-latest-save",
                            MenuButtonAction::Replay => "menu-replay",
                            MenuButtonAction::ReplayLatest => "menu-replay-latest",
                            MenuButtonAction::StartSkirmishNow => "menu-start-skirmish-now",
                            MenuButtonAction::DifficultyEasy => "menu-difficulty-easy",
                            MenuButtonAction::DifficultyMedium => "menu-difficulty-medium",
                            MenuButtonAction::DifficultyHard => "menu-difficulty-hard",
                            MenuButtonAction::ChallengeGeneral(0) => "menu-challenge-general-0",
                            MenuButtonAction::ChallengeGeneral(1) => "menu-challenge-general-1",
                            MenuButtonAction::ChallengeGeneral(2) => "menu-challenge-general-2",
                            MenuButtonAction::ChallengeGeneral(3) => "menu-challenge-general-3",
                            MenuButtonAction::ChallengeGeneral(4) => "menu-challenge-general-4",
                            MenuButtonAction::ChallengeGeneral(5) => "menu-challenge-general-5",
                            MenuButtonAction::ChallengeGeneral(6) => "menu-challenge-general-6",
                            MenuButtonAction::ChallengeGeneral(7) => "menu-challenge-general-7",
                            MenuButtonAction::ChallengeGeneral(8) => "menu-challenge-general-8",
                            MenuButtonAction::ChallengeGeneral(_) => "menu-challenge-general",
                            MenuButtonAction::ChallengeStart => "menu-challenge-start",
                            MenuButtonAction::OnlineQuickMatch => "menu-online-quickmatch",
                            MenuButtonAction::OnlineCustomMatch => "menu-online-custom",
                            MenuButtonAction::NetworkHost => "menu-network-host",
                            MenuButtonAction::NetworkJoin => "menu-network-join",
                            MenuButtonAction::TogglePause => "menu-toggle-pause",
                            MenuButtonAction::MainMenu => "menu-main-menu",
                        };
                        let danger = matches!(action, MenuButtonAction::ExitGame);
                        self.render_overlay_button(
                            id,
                            label,
                            action,
                            danger,
                            button_width,
                            button_height,
                            text_padding_left,
                            cx,
                        )
                    }))
                    .when(self.current_menu_screen() == "Challenge", |menu| {
                        let selected = CHALLENGE_GENERALS
                            .get(self.selected_challenge_general)
                            .map(|entry| entry.label)
                            .unwrap_or(CHALLENGE_GENERALS[0].label);
                        let difficulty = self
                            .selected_challenge_difficulty
                            .map(|value| value.label())
                            .unwrap_or("Not selected");
                        menu.child(
                            div()
                                .w(px(button_width))
                                .text_xs()
                                .text_color(rgb(0xd8c78c))
                                .child(format!("Challenge: {selected} | Difficulty: {difficulty}")),
                        )
                    })
                    .child(
                        div()
                            .w(px(button_width))
                            .text_xs()
                            .text_color(rgb(0xaec0d3))
                            .child(self.menu_panel_title()),
                    )
                    .when_some(self.menu_note.as_ref(), |menu, note| {
                        menu.child(
                            div()
                                .w(px(button_width))
                                .text_xs()
                                .text_color(rgb(0xaec0d3))
                                .child(note.clone()),
                        )
                    }),
            )
            .into_any_element()
    }
}

impl Render for EditorApp {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.runtime.poll();
        self.sync_menu_screen();
        let runtime_viewport_image = self.runtime.viewport_image();
        let viewport_size = window.viewport_size();
        let viewport_width = f32::from(viewport_size.width).max(1.0);
        let viewport_height = f32::from(viewport_size.height).max(1.0);
        window.request_animation_frame();

        div().size_full().bg(rgb(0x070b10)).child(
            div()
                .id("viewport-panel")
                .relative()
                .h_full()
                .w_full()
                .bg(rgb(0x0b1723))
                .flex()
                .items_center()
                .justify_center()
                .when_some(runtime_viewport_image.clone(), |panel, image| {
                    panel.child(img(image).h_full().w_full().max_w_full())
                })
                .child(self.runtime_overlay(viewport_width, viewport_height, _cx)),
        )
    }
}

#[derive(Clone)]
struct RuntimeStatus {
    state: String,
    ui_screen: String,
    paused: bool,
    fps: f32,
    startup_progress: f32,
    startup_phase: String,
    map: String,
    frame: u32,
}

impl Default for RuntimeStatus {
    fn default() -> Self {
        Self {
            state: "NotRunning".to_string(),
            ui_screen: "None".to_string(),
            paused: false,
            fps: 0.0,
            startup_progress: 0.0,
            startup_phase: "Loading assets".to_string(),
            map: "-".to_string(),
            frame: 0,
        }
    }
}

struct RuntimeBridge {
    child: Option<Child>,
    control_path: PathBuf,
    status_path: PathBuf,
    frame_path: PathBuf,
    frame_meta_path: PathBuf,
    status: RuntimeStatus,
    latest_frame_image: Option<Arc<Image>>,
    latest_frame_luma: f32,
    last_frame_modified: Option<SystemTime>,
    last_status_frame_seen: u32,
    last_status_frame_seen_at: Option<Instant>,
    last_live_frame_loaded_at: Option<Instant>,
    loaded_frame_count: u64,
    rejected_frame_count: u64,
    frame_read_error_count: u64,
    command_sequence: u64,
    launch_error: Option<String>,
    launched_at: Instant,
    last_no_frame_log_at: Option<Instant>,
    fallback_activated_at: Option<Instant>,
    last_health_log_at: Option<Instant>,
    last_dark_frame_warning_at: Option<Instant>,
}

impl RuntimeBridge {
    fn bytes_look_like_png(bytes: &[u8]) -> bool {
        bytes.len() >= 128
            && bytes
                .get(0..8)
                .map(|sig| sig == [137, 80, 78, 71, 13, 10, 26, 10])
                .unwrap_or(false)
    }

    fn estimate_luma_from_image_bytes(bytes: &[u8]) -> Option<f32> {
        let decoded = image::load_from_memory(bytes).ok()?;
        let rgba = decoded.to_rgba8();
        let raw = rgba.as_raw();
        if raw.is_empty() {
            return Some(0.0);
        }
        let pixels = raw.len() / 4;
        let stride = (pixels / 8192).max(1);
        let mut sum = 0.0f32;
        let mut count = 0usize;
        for index in (0..pixels).step_by(stride) {
            let base = index * 4;
            let r = raw[base] as f32;
            let g = raw[base + 1] as f32;
            let b = raw[base + 2] as f32;
            sum += 0.2126 * r + 0.7152 * g + 0.0722 * b;
            count += 1;
        }
        if count == 0 {
            Some(0.0)
        } else {
            Some((sum / count as f32).clamp(0.0, 255.0))
        }
    }

    fn runtime_executable() -> std::io::Result<PathBuf> {
        if let Some(explicit) = std::env::var_os("GENERALS_RUNTIME_EXE") {
            return Ok(PathBuf::from(explicit));
        }
        let current_exe = std::env::current_exe()?;
        let is_generals = current_exe
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(|stem| stem.eq_ignore_ascii_case("generals"))
            .unwrap_or(false);
        if is_generals {
            return Ok(current_exe);
        }

        if let Some(dir) = current_exe.parent() {
            for candidate in [
                dir.join("generals"),
                dir.join("generals.exe"),
                dir.join("generals_main"),
                dir.join("generals_main.exe"),
            ] {
                if candidate.is_file() {
                    return Ok(candidate);
                }
            }
        }

        for candidate in [
            PathBuf::from("GeneralsRust/target/debug/generals"),
            PathBuf::from("GeneralsRust/target/release/generals"),
            PathBuf::from("GeneralsRust/target/debug/generals.exe"),
            PathBuf::from("GeneralsRust/target/release/generals.exe"),
        ] {
            if candidate.is_file() {
                return Ok(candidate);
            }
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "could not locate generals runtime executable; set GENERALS_RUNTIME_EXE",
        ))
    }

    fn spawn_runtime_process(control_path: &Path, status_path: &Path) -> std::io::Result<Child> {
        let frame_path = status_path.with_extension("frame.png");
        let runtime_exe = Self::runtime_executable()?;
        Command::new(runtime_exe)
            .arg("-runtime_host")
            .arg("headless")
            .arg("-windowed")
            .arg("-gpui_control")
            .arg(control_path)
            .arg("-gpui_status")
            .arg(status_path)
            .arg("-gpui_frame")
            .arg(frame_path)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
    }

    fn try_spawn_runtime(&mut self) {
        if self.child.is_some() {
            return;
        }
        let _ = fs::remove_file(&self.control_path);
        let _ = fs::remove_file(&self.status_path);
        let _ = fs::remove_file(&self.frame_path);
        let _ = fs::remove_file(&self.frame_meta_path);
        match Self::spawn_runtime_process(&self.control_path, &self.status_path) {
            Ok(child) => {
                self.child = Some(child);
                self.status = RuntimeStatus {
                    state: "Booting".to_string(),
                    ..RuntimeStatus::default()
                };
                self.latest_frame_image = None;
                self.latest_frame_luma = 0.0;
                self.last_frame_modified = None;
                self.last_status_frame_seen = 0;
                self.last_status_frame_seen_at = None;
                self.last_live_frame_loaded_at = None;
                self.loaded_frame_count = 0;
                self.rejected_frame_count = 0;
                self.frame_read_error_count = 0;
                self.command_sequence = 0;
                self.launch_error = None;
                self.launched_at = Instant::now();
                self.last_no_frame_log_at = None;
                self.fallback_activated_at = None;
                self.last_health_log_at = None;
                self.last_dark_frame_warning_at = None;
                info!(
                    "GPUI runtime bridge launched runtime process (status={}, screen={})",
                    self.status.state, self.status.ui_screen
                );
            }
            Err(err) => {
                self.child = None;
                self.status.state = "LaunchFailed".to_string();
                self.launch_error = Some(err.to_string());
                warn!("GPUI runtime bridge failed to launch runtime process: {err}");
            }
        }
    }

    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let control_path =
            std::env::temp_dir().join(format!("generals_gpui_editor_control_{stamp}.txt"));
        let status_path =
            std::env::temp_dir().join(format!("generals_gpui_editor_status_{stamp}.txt"));
        let frame_path = status_path.with_extension("frame.png");
        let frame_meta_path = frame_path.with_extension("png.meta");
        let mut bridge = Self {
            child: None,
            control_path,
            status_path,
            frame_path,
            frame_meta_path,
            status: RuntimeStatus::default(),
            latest_frame_image: None,
            latest_frame_luma: 0.0,
            last_frame_modified: None,
            last_status_frame_seen: 0,
            last_status_frame_seen_at: None,
            last_live_frame_loaded_at: None,
            loaded_frame_count: 0,
            rejected_frame_count: 0,
            frame_read_error_count: 0,
            command_sequence: 0,
            launch_error: None,
            launched_at: Instant::now(),
            last_no_frame_log_at: None,
            fallback_activated_at: None,
            last_health_log_at: None,
            last_dark_frame_warning_at: None,
        };
        bridge.try_spawn_runtime();
        bridge
    }

    fn send_command(&mut self, command: &str) -> std::io::Result<()> {
        if let Some(child) = self.child.as_mut() {
            if child.try_wait().ok().flatten().is_some() {
                self.child = None;
            }
        }
        if self.child.is_none() {
            self.try_spawn_runtime();
        }
        if self.child.is_none() {
            return Err(std::io::Error::other(
                "runtime process is not running in GPUI host",
            ));
        }
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.control_path)?;
        self.command_sequence = self.command_sequence.saturating_add(1);
        let command = command.trim().replace('\n', " ");
        writeln!(file, "{}", command)?;
        file.flush()?;
        let _ = file.sync_data();
        info!(
            "GPUI runtime bridge command#{} -> {}",
            self.command_sequence, command
        );
        Ok(())
    }

    fn frame_age(&self) -> Option<Duration> {
        self.last_live_frame_loaded_at
            .map(|timestamp| timestamp.elapsed())
    }

    fn poll(&mut self) {
        if let Some(child) = self.child.as_mut() {
            if let Ok(Some(status)) = child.try_wait() {
                self.child = None;
                self.status.state = format!("Exited({})", status.code().unwrap_or_default());
            }
        }

        let Ok(payload) = fs::read_to_string(&self.status_path) else {
            return;
        };
        let previous_status = self.status.clone();
        let mut next = previous_status.clone();
        let mut runtime_frame_path: Option<PathBuf> = None;
        for line in payload.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key.trim() {
                "state" => next.state = value.trim().to_string(),
                "ui_screen" => next.ui_screen = value.trim().to_string(),
                "paused" => next.paused = value.trim().eq_ignore_ascii_case("true"),
                "fps" => next.fps = value.trim().parse().unwrap_or(next.fps),
                "startup_progress" => {
                    next.startup_progress = value.trim().parse().unwrap_or(next.startup_progress)
                }
                "startup_phase" => next.startup_phase = value.trim().to_string(),
                "map" => next.map = value.trim().to_string(),
                "frame" => next.frame = value.trim().parse().unwrap_or(next.frame),
                "frame_path" => {
                    let candidate = value.trim();
                    if !candidate.is_empty() {
                        runtime_frame_path = Some(PathBuf::from(candidate));
                    }
                }
                _ => {}
            }
        }
        if let Some(path) = runtime_frame_path {
            self.frame_path = path;
            self.frame_meta_path = self.frame_path.with_extension("png.meta");
        }
        self.status = next;
        if self.status.frame != self.last_status_frame_seen {
            self.last_status_frame_seen = self.status.frame;
            self.last_status_frame_seen_at = Some(Instant::now());
        }
        if self.status.state != previous_status.state
            || self.status.ui_screen != previous_status.ui_screen
        {
            info!(
                "GPUI runtime bridge status changed: state={} ui={} frame={}",
                self.status.state, self.status.ui_screen, self.status.frame
            );
        }
        if let Ok(meta) = fs::read_to_string(&self.frame_meta_path) {
            for line in meta.lines() {
                let Some((key, value)) = line.split_once('=') else {
                    continue;
                };
                if key.trim() == "luma" {
                    if let Ok(luma) = value.trim().parse::<f32>() {
                        self.latest_frame_luma = luma.max(0.0);
                    }
                }
            }
        }
        let should_reload_frame = fs::metadata(&self.frame_path)
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .map(|modified| {
                if self
                    .last_frame_modified
                    .is_some_and(|previous| previous >= modified)
                {
                    false
                } else {
                    self.last_frame_modified = Some(modified);
                    true
                }
            })
            .unwrap_or(false);
        if should_reload_frame {
            if let Ok(bytes) = fs::read(&self.frame_path) {
                if Self::bytes_look_like_png(&bytes) {
                    let first_frame = self.latest_frame_image.is_none();
                    if let Some(luma) = Self::estimate_luma_from_image_bytes(&bytes) {
                        self.latest_frame_luma = luma;
                    }
                    self.latest_frame_image =
                        Some(Arc::new(Image::from_bytes(ImageFormat::Png, bytes)));
                    self.loaded_frame_count = self.loaded_frame_count.saturating_add(1);
                    self.last_live_frame_loaded_at = Some(Instant::now());
                    if first_frame {
                        info!(
                            "GPUI runtime bridge received first live viewport frame (runtime_frame={})",
                            self.status.frame
                        );
                    }
                } else {
                    self.rejected_frame_count = self.rejected_frame_count.saturating_add(1);
                    warn!(
                        "GPUI runtime bridge rejected runtime frame bytes (size={}, path={})",
                        bytes.len(),
                        self.frame_path.display()
                    );
                }
            } else {
                self.frame_read_error_count = self.frame_read_error_count.saturating_add(1);
            }
        }

        let visible = self.has_visible_frame();
        let runtime_is_interactive = matches!(self.status.state.as_str(), "Menu" | "Playing" | "Paused");
        if visible
            && runtime_is_interactive
            && self.loaded_frame_count >= 4
            && self.latest_frame_luma <= 2.0
        {
            let should_warn_dark = self
                .last_dark_frame_warning_at
                .map(|last| last.elapsed() >= Duration::from_secs(2))
                .unwrap_or(true);
            if should_warn_dark {
                warn!(
                    "GPUI runtime bridge detected very dark live frames (luma={:.2}, state={}, ui={}, frame={}, loaded={})",
                    self.latest_frame_luma,
                    self.status.state,
                    self.status.ui_screen,
                    self.status.frame,
                    self.loaded_frame_count
                );
                self.last_dark_frame_warning_at = Some(Instant::now());
            }
        }

        if !visible {
            if self.fallback_activated_at.is_none() {
                self.fallback_activated_at = Some(Instant::now());
                info!(
                    "GPUI runtime bridge activated fallback viewport (state={} ui={} frame={})",
                    self.status.state, self.status.ui_screen, self.status.frame
                );
            }
        } else if let Some(activated_at) = self.fallback_activated_at.take() {
            info!(
                "GPUI runtime bridge deactivated fallback viewport after {:.2}s",
                activated_at.elapsed().as_secs_f32()
            );
        }

        if self.latest_frame_image.is_none()
            && self.launched_at.elapsed() >= std::time::Duration::from_secs(3)
            && self
                .last_no_frame_log_at
                .map(|last| last.elapsed() >= std::time::Duration::from_secs(2))
                .unwrap_or(true)
        {
            warn!(
                "GPUI runtime bridge waiting for viewport frame: state={} ui={} frame={} status_path={} frame_path={}",
                self.status.state,
                self.status.ui_screen,
                self.status.frame,
                self.status_path.display(),
                self.frame_path.display()
            );
            self.last_no_frame_log_at = Some(Instant::now());
        }

        let should_log_health = self
            .last_health_log_at
            .map(|last| last.elapsed() >= Duration::from_secs(2))
            .unwrap_or(true);
        if should_log_health {
            let frame_age_ms = self.frame_age().map(|age| age.as_millis()).unwrap_or(0);
            let status_age_ms = self
                .last_status_frame_seen_at
                .map(|age| age.elapsed().as_millis())
                .unwrap_or(0);
            info!(
                "GPUI runtime bridge health: state={} ui={} runtime_frame={} loaded={} rejected={} read_errors={} frame_age_ms={} status_frame_age_ms={} fallback_active={} visible={} luma={:.2}",
                self.status.state,
                self.status.ui_screen,
                self.status.frame,
                self.loaded_frame_count,
                self.rejected_frame_count,
                self.frame_read_error_count,
                frame_age_ms,
                status_age_ms,
                self.fallback_activated_at.is_some(),
                visible,
                self.latest_frame_luma,
            );
            self.last_health_log_at = Some(Instant::now());
        }
    }

    fn status_line(&self) -> String {
        if let Some(err) = &self.launch_error {
            return format!("Runtime error: {err}");
        }
        if self.child.is_none() {
            return "Runtime process stopped (next action restarts it)".to_string();
        }
        if self.status.state == "Booting" {
            return "Runtime booting...".to_string();
        }
        format!(
            "State={} | UI={} | Phase={} | FPS={:.1} | Load={:.0}% | Map={} | Frame={} | FrameAge={}ms",
            self.status.state,
            self.status.ui_screen,
            self.status.startup_phase,
            self.status.fps,
            self.status.startup_progress * 100.0,
            self.status.map,
            self.status.frame,
            self.frame_age().map(|age| age.as_millis()).unwrap_or(0)
        )
    }

    fn viewport_image(&self) -> Option<Arc<Image>> {
        self.latest_frame_image.clone()
    }

    fn has_visible_frame(&self) -> bool {
        self.latest_frame_image.is_some()
            && self.status.frame > 0
            && !matches!(self.status.state.as_str(), "NotRunning" | "LaunchFailed")
    }

    fn state(&self) -> &str {
        &self.status.state
    }

    fn ui_screen_name(&self) -> Option<&str> {
        let raw = self.status.ui_screen.trim();
        let inner = raw.strip_prefix("Some(")?.strip_suffix(')')?;
        if inner.is_empty() {
            None
        } else {
            Some(inner)
        }
    }

    fn on_main_menu_screen(&self) -> bool {
        matches!(self.ui_screen_name(), Some("MainMenu"))
    }

    fn latest_slot_in_dir(&self, relative_dir: &str, extension: &str) -> Option<String> {
        let runtime_exe = Self::runtime_executable().ok()?;
        let save_dir = runtime_exe.parent()?.join(relative_dir);
        let entries = fs::read_dir(save_dir).ok()?;

        let mut latest: Option<(std::time::SystemTime, String)> = None;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some(extension) {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            let modified = entry
                .metadata()
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            match &latest {
                Some((current_modified, _)) if *current_modified >= modified => {}
                _ => latest = Some((modified, stem.to_string())),
            }
        }

        latest.map(|(_, slot)| slot)
    }

    fn latest_save_slot(&self) -> Option<String> {
        self.latest_slot_in_dir("Save Games", "gen")
    }

    fn latest_replay_slot(&self) -> Option<String> {
        self.latest_slot_in_dir("Save Games/Replays", "rep")
    }

    fn startup_progress(&self) -> f32 {
        self.status.startup_progress.clamp(0.0, 1.0)
    }

    fn startup_phase(&self) -> &str {
        let phase = self.status.startup_phase.trim();
        if phase.is_empty() {
            "Loading assets"
        } else {
            phase
        }
    }
}

impl Drop for RuntimeBridge {
    fn drop(&mut self) {
        let _ = fs::write(&self.control_path, "exit\n");
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = fs::remove_file(&self.control_path);
        let _ = fs::remove_file(&self.status_path);
        let _ = fs::remove_file(&self.frame_path);
        let _ = fs::remove_file(&self.frame_meta_path);
    }
}
