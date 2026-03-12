//! Shim for WOLBuddyOverlay.cpp callbacks.

use crate::gui::callbacks::wol_buddy_overlay::{
    popup_buddy_notification_system, wol_buddy_overlay_init, wol_buddy_overlay_input,
    wol_buddy_overlay_rc_menu_init, wol_buddy_overlay_rc_menu_system, wol_buddy_overlay_shutdown,
    wol_buddy_overlay_system, wol_buddy_overlay_update,
};
use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn WOLBuddyOverlayInit(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_buddy_overlay_init(layout, user_data);
}

pub fn WOLBuddyOverlayUpdate(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_buddy_overlay_update(layout, user_data);
}

pub fn WOLBuddyOverlayShutdown(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_buddy_overlay_shutdown(layout, user_data);
}

pub fn WOLBuddyOverlaySystem(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    wol_buddy_overlay_system(window, msg, data1, data2)
}

pub fn WOLBuddyOverlayInput(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    wol_buddy_overlay_input(window, msg, data1, data2)
}

pub fn PopupBuddyNotificationSystem(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    popup_buddy_notification_system(window, msg, data1, data2)
}

pub fn WOLBuddyOverlayRCMenuInit(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    wol_buddy_overlay_rc_menu_init(layout, user_data);
}

pub fn WOLBuddyOverlayRCMenuSystem(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    wol_buddy_overlay_rc_menu_system(window, msg, data1, data2)
}
