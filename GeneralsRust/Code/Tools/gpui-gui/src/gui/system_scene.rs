use gpui::{div, prelude::*, px, rgb, AnyElement};

use crate::gui::animate_window_manager::AnimateWindowManagerPort;
use crate::gui::challenge_generals::ChallengeGeneralsPort;
use crate::gui::disconnect_menu::DisconnectMenuPort;
use crate::gui::establish_connections_menu::EstablishConnectionsPort;
use crate::gui::game_font::FontLibraryPort;
use crate::gui::game_window_global::GameWindowGlobalPort;
use crate::gui::game_window_manager_script::WindowScriptPort;
use crate::gui::game_window_transitions::TransitionHandlerPort;
use crate::gui::game_window_transitions_styles;
use crate::gui::header_template::HeaderTemplateManagerPort;
use crate::gui::ime_manager::ImeManagerPort;
use crate::gui::load_screen::LoadScreenPort;
use crate::gui::process_animate_window::{AnimationKindPort, ProcessAnimateWindowPort};
use crate::gui::win_instance_data::WinInstanceDataPort;
use crate::gui::window_video_manager::{WindowVideoManagerPort, WindowVideoPlayTypePort};

pub fn render_system_cards() -> Vec<AnyElement> {
    let challenge = ChallengeGeneralsPort::init_defaults();
    let mut fonts = FontLibraryPort::defaults();
    let headers = HeaderTemplateManagerPort::init_defaults();
    let mut ime = ImeManagerPort::default();
    ime.attach(1001);
    ime.set_composition("han", 2);
    let mut load = LoadScreenPort::default();
    load.start_ambient_loop();
    load.update_percent(61);
    load.status_text = "Streaming textures".to_string();
    for frame in [251, 255, 356, 358, 360, 434, 464] {
        load.tick_frame(frame);
    }
    let mut video = WindowVideoManagerPort::default();
    video.play(1002, "IntroMovie.bik", WindowVideoPlayTypePort::Loop);
    let globals = GameWindowGlobalPort::default();
    let script = WindowScriptPort::from_layout("Menus/MainMenu.wnd", 3);
    let mut transitions = TransitionHandlerPort::default();
    transitions.trigger("FlashTransition", 12);
    let mut animator = AnimateWindowManagerPort::default();
    animator.register("SlideFromRight");
    let process = ProcessAnimateWindowPort {
        kind: AnimationKindPort::SlideFromRight,
        delay_ms: 120,
    };
    let disconnect = DisconnectMenuPort::default();
    let establish = EstablishConnectionsPort::default();
    let instance = WinInstanceDataPort::default();

    vec![
        panel(
            "ChallengeGenerals.cpp",
            vec![
                line("Personas", challenge.personas.len().to_string()),
                line(
                    "Active",
                    challenge
                        .get_player_general_by_campaign_name("BossGeneral")
                        .map(|persona| persona.display_name.clone())
                        .unwrap_or_else(|| "None".to_string()),
                ),
            ],
        ),
        panel(
            "GameFont.cpp / HeaderTemplate.cpp",
            vec![
                line("Fonts", fonts.fonts.len().to_string()),
                line("Headers", headers.templates.len().to_string()),
                line(
                    "Resolved Fonts",
                    headers.populate_game_fonts(&mut fonts).len().to_string(),
                ),
            ],
        ),
        panel(
            "IMEManager.cpp",
            vec![
                line(
                    "Attached Window",
                    ime.attached_window.unwrap_or_default().to_string(),
                ),
                line("Composition", ime.composition_string.clone()),
            ],
        ),
        panel(
            "LoadScreen.cpp",
            vec![
                line("Status", load.status_text.clone()),
                line("Progress", load.percent_text.clone()),
                line("Briefing", load.briefing_voice_started.to_string()),
                line("Location", load.location_visible.to_string()),
                line("Visible Cameos", load.visible_cameo_count().to_string()),
            ],
        ),
        panel(
            "WindowVideoManager.cpp",
            vec![
                line("Active Videos", video.videos.len().to_string()),
                line(
                    "Movie",
                    video
                        .videos
                        .first()
                        .map(|video| video.movie_name.clone())
                        .unwrap_or_else(|| "None".to_string()),
                ),
            ],
        ),
        panel(
            "GameWindowGlobal.cpp / Script / Transitions",
            vec![
                line("Mouse Pos", globals.send_mouse_pos_messages.to_string()),
                line("Script", script.filename),
                line(
                    "Styles",
                    game_window_transitions_styles::default_styles()
                        .len()
                        .to_string(),
                ),
                line("Groups", transitions.groups.len().to_string()),
                line(
                    "Current",
                    transitions
                        .current_group
                        .clone()
                        .unwrap_or_else(|| "None".to_string()),
                ),
            ],
        ),
        panel(
            "AnimateWindowManager.cpp / ProcessAnimateWindow.cpp",
            vec![
                line("Queued", animator.is_finished().not().to_string()),
                line("Kind", format!("{:?}", process.kind)),
                line("Delay", format!("{}ms", process.delay_ms)),
            ],
        ),
        panel(
            "Disconnect / Establish Connections",
            vec![
                line("Disconnect", disconnect.headline),
                line("Connect Stage", establish.stage),
                line("Peers", establish.peers_connected.to_string()),
            ],
        ),
        panel(
            "WinInstanceData.cpp",
            vec![
                line("Tooltip Delay", instance.tooltip_delay.to_string()),
                line(
                    "Enabled Draw Slots",
                    instance.enabled_draw_data.len().to_string(),
                ),
            ],
        ),
    ]
}

fn panel(title: &str, lines: Vec<AnyElement>) -> AnyElement {
    div()
        .w(px(260.))
        .p_3()
        .rounded_lg()
        .border_1()
        .border_color(rgb(0x22303f))
        .bg(rgb(0x0e1620))
        .flex()
        .flex_col()
        .gap_2()
        .child(title.to_string())
        .children(lines)
        .into_any_element()
}

fn line(label: &str, value: String) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x8ea2b4))
                .child(label.to_string()),
        )
        .child(value)
        .into_any_element()
}

trait BoolExt {
    fn not(self) -> bool;
}

impl BoolExt for bool {
    fn not(self) -> bool {
        !self
    }
}
