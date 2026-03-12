use crate::gui::callbacks::wol_lobby_menu::{
    wol_lobby_menu_init, wol_lobby_menu_input, wol_lobby_menu_shutdown, wol_lobby_menu_system,
    wol_lobby_menu_update,
};
use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn wol_lobby_menu_init_callback(
    layout: &WindowLayout,
    user_data: Option<&mut dyn std::any::Any>,
) {
    wol_lobby_menu_init(layout, user_data);
}

pub fn wol_lobby_menu_update_callback(
    layout: &WindowLayout,
    user_data: Option<&mut dyn std::any::Any>,
) {
    wol_lobby_menu_update(layout, user_data);
}

pub fn wol_lobby_menu_shutdown_callback(
    layout: &WindowLayout,
    user_data: Option<&mut dyn std::any::Any>,
) {
    wol_lobby_menu_shutdown(layout, user_data);
}

pub fn wol_lobby_menu_system_callback(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    wol_lobby_menu_system(window, msg, data1, data2)
}

pub fn wol_lobby_menu_input_callback(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    wol_lobby_menu_input(window, msg, data1, data2)
}
