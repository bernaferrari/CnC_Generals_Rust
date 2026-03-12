//! No-op online callback stubs for builds without `network`.
//!
//! The non-network milestone keeps offline/skirmish gameplay compiling while
//! GameSpy/LAN callback parity is finished behind the feature gate.

use std::any::Any;

use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

macro_rules! layout_noop {
    ($($name:ident),+ $(,)?) => {
        $(
            pub fn $name(_layout: &WindowLayout, _user_data: Option<&dyn Any>) {}
        )+
    };
}

macro_rules! system_ignored {
    ($($name:ident),+ $(,)?) => {
        $(
            pub fn $name(
                _window: &GameWindow,
                _msg: WindowMessage,
                _data1: WindowMsgData,
                _data2: WindowMsgData,
            ) -> WindowMsgHandled {
                WindowMsgHandled::Ignored
            }
        )+
    };
}

layout_noop!(
    popup_join_game_init,
    popup_host_game_init,
    popup_host_game_update,
    popup_ladder_select_init,
    popup_ladder_select_update,
    popup_ladder_select_shutdown,
    popup_player_info_init,
    popup_player_info_update,
    popup_player_info_shutdown,
    network_direct_connect_init,
    network_direct_connect_update,
    network_direct_connect_shutdown,
    wol_locale_select_init,
    wol_locale_select_update,
    wol_locale_select_shutdown,
    wol_custom_score_screen_init,
    wol_custom_score_screen_update,
    wol_custom_score_screen_shutdown,
    wol_message_window_init,
    wol_message_window_update,
    wol_message_window_shutdown,
    wol_buddy_overlay_init,
    wol_buddy_overlay_update,
    wol_buddy_overlay_shutdown,
    wol_buddy_overlay_rc_menu_init,
    wol_lobby_menu_init,
    wol_lobby_menu_update,
    wol_lobby_menu_shutdown,
    wol_ladder_screen_init,
    wol_ladder_screen_update,
    wol_ladder_screen_shutdown,
    wol_game_setup_menu_init,
    wol_game_setup_menu_update,
    wol_game_setup_menu_shutdown,
    wol_quick_match_menu_init,
    wol_quick_match_menu_update,
    wol_quick_match_menu_shutdown,
    wol_map_select_menu_init,
    wol_map_select_menu_update,
    wol_map_select_menu_shutdown,
    wol_login_menu_init,
    wol_login_menu_update,
    wol_login_menu_shutdown,
    wol_welcome_menu_init,
    wol_welcome_menu_update,
    wol_welcome_menu_shutdown,
    wol_qm_score_screen_init,
    wol_qm_score_screen_update,
    wol_qm_score_screen_shutdown,
    wol_status_menu_init,
    wol_status_menu_update,
    wol_status_menu_shutdown,
    rc_game_details_menu_init,
);

system_ignored!(
    popup_join_game_input,
    popup_join_game_system,
    popup_host_game_input,
    popup_host_game_system,
    popup_ladder_select_input,
    popup_ladder_select_system,
    popup_player_info_input,
    popup_player_info_system,
    network_direct_connect_input,
    network_direct_connect_system,
    wol_locale_select_input,
    wol_locale_select_system,
    wol_custom_score_screen_input,
    wol_custom_score_screen_system,
    wol_message_window_input,
    wol_message_window_system,
    wol_buddy_overlay_input,
    wol_buddy_overlay_system,
    popup_buddy_notification_system,
    wol_buddy_overlay_rc_menu_system,
    wol_lobby_menu_input,
    wol_lobby_menu_system,
    wol_ladder_screen_input,
    wol_ladder_screen_system,
    wol_game_setup_menu_input,
    wol_game_setup_menu_system,
    wol_quick_match_menu_input,
    wol_quick_match_menu_system,
    wol_map_select_menu_input,
    wol_map_select_menu_system,
    wol_login_menu_input,
    wol_login_menu_system,
    wol_welcome_menu_input,
    wol_welcome_menu_system,
    wol_qm_score_screen_input,
    wol_qm_score_screen_system,
    wol_status_menu_input,
    wol_status_menu_system,
    rc_game_details_menu_system,
);
