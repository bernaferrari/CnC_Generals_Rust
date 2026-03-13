pub mod challenge_menu;
pub mod credits_menu;
pub mod difficulty_select;
pub mod disconnect_window;
pub mod download_menu;
pub mod establish_connections_window;
pub mod game_info_window;
pub mod keyboard_options_menu;
pub mod lan_game_options_menu;
pub mod lan_lobby_menu;
pub mod lan_map_select_menu;
pub mod main_menu;
pub mod map_select_menu;
pub mod network_direct_connect;
pub mod options_menu;
pub mod popup_communicator;
pub mod popup_host_game;
pub mod popup_join_game;
pub mod popup_ladder_select;
pub mod popup_player_info;
pub mod popup_replay;
pub mod popup_save_load;
pub mod quit_menu;
pub mod replay_menu;
pub mod scene;
pub mod score_screen;
pub mod single_player_menu;
pub mod skirmish_game_options_menu;
pub mod skirmish_map_select_menu;
pub mod wol_buddy_overlay;
pub mod wol_custom_score_screen;
pub mod wol_game_setup_menu;
pub mod wol_ladder_screen;
pub mod wol_lobby_menu;
pub mod wol_locale_select_popup;
pub mod wol_login_menu;
pub mod wol_map_select_menu;
pub mod wol_message_window;
pub mod wol_qm_score_screen;
pub mod wol_quick_match_menu;
pub mod wol_status_menu;
pub mod wol_welcome_menu;

use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};

pub fn records() -> Vec<&'static GuiPortRecord> {
    vec![
        &challenge_menu::RECORD,
        &credits_menu::RECORD,
        &difficulty_select::RECORD,
        &disconnect_window::RECORD,
        &download_menu::RECORD,
        &establish_connections_window::RECORD,
        &game_info_window::RECORD,
        &keyboard_options_menu::RECORD,
        &lan_game_options_menu::RECORD,
        &lan_lobby_menu::RECORD,
        &lan_map_select_menu::RECORD,
        &main_menu::RECORD,
        &map_select_menu::RECORD,
        &network_direct_connect::RECORD,
        &options_menu::RECORD,
        &popup_communicator::RECORD,
        &popup_host_game::RECORD,
        &popup_join_game::RECORD,
        &popup_ladder_select::RECORD,
        &popup_player_info::RECORD,
        &popup_replay::RECORD,
        &popup_save_load::RECORD,
        &quit_menu::RECORD,
        &replay_menu::RECORD,
        &score_screen::RECORD,
        &single_player_menu::RECORD,
        &skirmish_game_options_menu::RECORD,
        &skirmish_map_select_menu::RECORD,
        &wol_buddy_overlay::RECORD,
        &wol_custom_score_screen::RECORD,
        &wol_game_setup_menu::RECORD,
        &wol_ladder_screen::RECORD,
        &wol_lobby_menu::RECORD,
        &wol_locale_select_popup::RECORD,
        &wol_login_menu::RECORD,
        &wol_map_select_menu::RECORD,
        &wol_message_window::RECORD,
        &wol_qm_score_screen::RECORD,
        &wol_quick_match_menu::RECORD,
        &wol_status_menu::RECORD,
        &wol_welcome_menu::RECORD,
    ]
}

pub fn ports() -> &'static [MenuScreenPort] {
    &[
        main_menu::SCREEN,
        single_player_menu::SCREEN,
        options_menu::SCREEN,
        map_select_menu::SCREEN,
        replay_menu::SCREEN,
        credits_menu::SCREEN,
        score_screen::SCREEN,
        popup_save_load::SCREEN,
        popup_communicator::SCREEN,
        skirmish_game_options_menu::SCREEN,
        skirmish_map_select_menu::SCREEN,
        challenge_menu::SCREEN,
        keyboard_options_menu::SCREEN,
        lan_lobby_menu::SCREEN,
        lan_game_options_menu::SCREEN,
        lan_map_select_menu::SCREEN,
        game_info_window::SCREEN,
        download_menu::SCREEN,
        difficulty_select::SCREEN,
        wol_ladder_screen::SCREEN,
        wol_login_menu::SCREEN,
        wol_locale_select_popup::SCREEN,
        wol_message_window::SCREEN,
        wol_quick_match_menu::SCREEN,
        wol_welcome_menu::SCREEN,
        wol_status_menu::SCREEN,
        wol_qm_score_screen::SCREEN,
        wol_lobby_menu::SCREEN,
        wol_game_setup_menu::SCREEN,
        wol_map_select_menu::SCREEN,
        wol_buddy_overlay::SCREEN,
        wol_custom_score_screen::SCREEN,
        popup_host_game::SCREEN,
        popup_join_game::SCREEN,
        popup_ladder_select::SCREEN,
        popup_player_info::SCREEN,
        popup_replay::SCREEN,
        quit_menu::SCREEN,
        network_direct_connect::SCREEN,
        disconnect_window::SCREEN,
        establish_connections_window::SCREEN,
    ]
}

pub fn screen(key: &str) -> Option<MenuScreenPort> {
    ports().iter().find(|screen| screen.key == key).copied()
}

pub fn render_screen(key: &str) -> gpui::AnyElement {
    match screen(key) {
        Some(screen) => scene::render_screen(screen),
        None => scene::render_screen(main_menu::SCREEN),
    }
}
