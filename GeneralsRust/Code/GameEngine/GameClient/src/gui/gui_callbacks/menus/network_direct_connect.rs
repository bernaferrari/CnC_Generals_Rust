//! Shim for NetworkDirectConnect.cpp callbacks.
#![allow(non_snake_case)]

use crate::gui::callbacks::network_direct_connect::{
    network_direct_connect_init, network_direct_connect_input, network_direct_connect_shutdown,
    network_direct_connect_system, network_direct_connect_update,
};
use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

pub fn NetworkDirectConnectInit(layout: &WindowLayout, user_data: Option<&mut dyn std::any::Any>) {
    network_direct_connect_init(layout, user_data);
}

pub fn NetworkDirectConnectUpdate(
    layout: &WindowLayout,
    user_data: Option<&mut dyn std::any::Any>,
) {
    network_direct_connect_update(layout, user_data);
}

pub fn NetworkDirectConnectShutdown(
    layout: &WindowLayout,
    user_data: Option<&mut dyn std::any::Any>,
) {
    network_direct_connect_shutdown(layout, user_data);
}

pub fn NetworkDirectConnectSystem(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    network_direct_connect_system(window, msg, data1, data2)
}

pub fn NetworkDirectConnectInput(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    network_direct_connect_input(window, msg, data1, data2)
}
