//! Bounded native ButtonLarge notification; never published or deferred a frame.
use super::WindowManager;
use crate::gui::callbacks::control_bar_callbacks::ControlBarViewport;
use crate::gui::control_bar::{ControlBar, ControlBarStage};
use crate::gui::game_window::{
    GameWindow, WindowMessage, WindowMsgData, WindowMsgHandled, WindowStatus,
};
use game_engine::common::name_key_generator::NameKeyGenerator;
use std::cell::RefCell;
use std::rc::Rc;

pub struct ControlBarInputContext<'a> {
    pub control_bar: &'a mut ControlBar,
    pub viewport: &'a mut dyn ControlBarViewport,
    pub replay: bool,
    pub game_ending: bool,
}
impl WindowManager {
    pub(super) fn send_input_selected(
        &mut self,
        window: &Rc<RefCell<GameWindow>>,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        routed: bool,
        context: Option<&mut ControlBarInputContext<'_>>,
    ) -> WindowMsgHandled {
        let Some(context) = context else {
            return if routed {
                window
                    .borrow_mut()
                    .send_routed_input_message(msg, data1, data2)
            } else {
                window.borrow_mut().send_input_message(msg, data1, data2)
            };
        };
        let (handled, notification) = {
            window
                .borrow_mut()
                .send_input_with_control_bar_notification(msg, data1, data2)
        };
        if let Some(notification) = notification {
            // Both lookup and completion use this receiving manager. Neither
            // re-enters the singleton while its outer RefMut is live.
            let owner_destroyed = notification
                .owner
                .borrow()
                .get_status()
                .contains(WindowStatus::DESTROYED);
            if !context.game_ending && !context.replay && !owner_destroyed {
                let parent_id =
                    NameKeyGenerator::name_to_key("ControlBar.wnd:ControlBarParent") as i32;
                let marker_id =
                    NameKeyGenerator::name_to_key("ControlBar.wnd:BackgroundMarker") as i32;
                let marker = self.get_window_by_id(marker_id);
                if let Some(parent) = self.get_window_by_id(parent_id) {
                    // Valid WND layout has a distinct master parent and button.
                    // A malformed self-parent cannot be mutably borrowed twice.
                    if !Rc::ptr_eq(&parent, window) {
                        if context.control_bar.get_control_bar_stage() != ControlBarStage::Default {
                            ControlBar::reveal_ingame_command_window_owned(self);
                        }
                        let parent_handle = Rc::clone(&parent);
                        let mut parent = parent.borrow_mut();
                        let mut button = window.borrow_mut();
                        context
                            .control_bar
                            .toggle_control_bar_stage_with_button_owned(
                                context.viewport,
                                context.replay,
                                Some(&mut parent),
                                Some(&mut button),
                            );
                        // The stage captured its authored default position. The
                        // marker helper walks the already-borrowed parent and
                        // adjusts the moved stage back to that default origin.
                        if let Some(marker) = marker.as_ref().filter(|marker| {
                            !Rc::ptr_eq(marker, window) && !Rc::ptr_eq(marker, &parent_handle)
                        }) {
                            context
                                .control_bar
                                .capture_control_bar_marker_owned(&marker.borrow(), &parent);
                        }
                    }
                }
            }
        }
        handled
    }
}
