//! Message Box Callbacks
//!
//! This module handles message box and dialog callbacks including
//! standard message boxes, extended message boxes, and quit dialogs.

use crate::gui::{
    with_window_manager, write_input_focus_response, GameWindow, WindowId, WindowMessage,
    WindowMsgData, WindowMsgHandled,
};
use game_engine::common::name_key_generator::NameKeyGenerator;
use log::{debug, info, warn};
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, RwLock};

const MSG_BOX_YES: u16 = 0x01;
const MSG_BOX_NO: u16 = 0x02;
const MSG_BOX_CANCEL: u16 = 0x04;
const MSG_BOX_OK: u16 = 0x08;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageBoxReturnType {
    Close,
    KeepOpen,
}

pub type MessageBoxFunc = Box<dyn FnMut()>;
pub type ExtendedMessageBoxFunc = Box<dyn FnMut(Option<&mut dyn Any>) -> MessageBoxReturnType>;

#[derive(Default)]
pub struct WindowMessageBoxData {
    yes_callback: RefCell<Option<MessageBoxFunc>>,
    no_callback: RefCell<Option<MessageBoxFunc>>,
    ok_callback: RefCell<Option<MessageBoxFunc>>,
    cancel_callback: RefCell<Option<MessageBoxFunc>>,
}

impl WindowMessageBoxData {
    pub fn new(
        yes_callback: Option<MessageBoxFunc>,
        no_callback: Option<MessageBoxFunc>,
        ok_callback: Option<MessageBoxFunc>,
        cancel_callback: Option<MessageBoxFunc>,
    ) -> Self {
        Self {
            yes_callback: RefCell::new(yes_callback),
            no_callback: RefCell::new(no_callback),
            ok_callback: RefCell::new(ok_callback),
            cancel_callback: RefCell::new(cancel_callback),
        }
    }

    fn call_yes(&self) {
        if let Some(callback) = self.yes_callback.borrow_mut().as_mut() {
            callback();
        }
    }

    fn call_no(&self) {
        if let Some(callback) = self.no_callback.borrow_mut().as_mut() {
            callback();
        }
    }

    fn call_ok(&self) {
        if let Some(callback) = self.ok_callback.borrow_mut().as_mut() {
            callback();
        }
    }

    fn call_cancel(&self) {
        if let Some(callback) = self.cancel_callback.borrow_mut().as_mut() {
            callback();
        }
    }
}

#[derive(Default)]
pub struct WindowExMessageBoxData {
    yes_callback: RefCell<Option<ExtendedMessageBoxFunc>>,
    no_callback: RefCell<Option<ExtendedMessageBoxFunc>>,
    ok_callback: RefCell<Option<ExtendedMessageBoxFunc>>,
    cancel_callback: RefCell<Option<ExtendedMessageBoxFunc>>,
    user_data: RefCell<Option<Box<dyn Any>>>,
}

impl WindowExMessageBoxData {
    pub fn new(
        yes_callback: Option<ExtendedMessageBoxFunc>,
        no_callback: Option<ExtendedMessageBoxFunc>,
        ok_callback: Option<ExtendedMessageBoxFunc>,
        cancel_callback: Option<ExtendedMessageBoxFunc>,
        user_data: Option<Box<dyn Any>>,
    ) -> Self {
        Self {
            yes_callback: RefCell::new(yes_callback),
            no_callback: RefCell::new(no_callback),
            ok_callback: RefCell::new(ok_callback),
            cancel_callback: RefCell::new(cancel_callback),
            user_data: RefCell::new(user_data),
        }
    }

    fn call_yes(&self) -> MessageBoxReturnType {
        if let Some(callback) = self.yes_callback.borrow_mut().as_mut() {
            return callback(self.user_data.borrow_mut().as_deref_mut());
        }
        MessageBoxReturnType::Close
    }

    fn call_no(&self) -> MessageBoxReturnType {
        if let Some(callback) = self.no_callback.borrow_mut().as_mut() {
            return callback(self.user_data.borrow_mut().as_deref_mut());
        }
        MessageBoxReturnType::Close
    }

    fn call_ok(&self) -> MessageBoxReturnType {
        if let Some(callback) = self.ok_callback.borrow_mut().as_mut() {
            return callback(self.user_data.borrow_mut().as_deref_mut());
        }
        MessageBoxReturnType::Close
    }

    fn call_cancel(&self) -> MessageBoxReturnType {
        if let Some(callback) = self.cancel_callback.borrow_mut().as_mut() {
            return callback(self.user_data.borrow_mut().as_deref_mut());
        }
        MessageBoxReturnType::Close
    }
}

/// Message box button types
#[derive(Debug, Clone, PartialEq)]
pub enum MessageBoxButton {
    Ok,
    Cancel,
    Yes,
    No,
    Retry,
    Abort,
    Ignore,
}

/// Message box result
#[derive(Debug, Clone, PartialEq)]
pub enum MessageBoxResult {
    Ok,
    Cancel,
    Yes,
    No,
    Retry,
    Abort,
    Ignore,
    Closed,
}

/// Message box types
#[derive(Debug, Clone, PartialEq)]
pub enum MessageBoxType {
    Ok,
    OkCancel,
    YesNo,
    YesNoCancel,
    RetryCancel,
    AbortRetryIgnore,
}

/// Standard message box implementation
pub struct MessageBoxCallbacks {
    visible: bool,
    message: String,
    title: String,
    message_type: MessageBoxType,
    result: Option<MessageBoxResult>,
    window_id: Option<WindowId>,
}

impl MessageBoxCallbacks {
    pub fn new() -> Self {
        Self {
            visible: false,
            message: String::new(),
            title: String::new(),
            message_type: MessageBoxType::Ok,
            result: None,
            window_id: None,
        }
    }

    /// Handle message box system messages
    pub fn system(
        &mut self,
        window: &GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        debug!(
            "Message box system message: {:?} for window: {}",
            msg,
            window.get_name()
        );

        match msg {
            WindowMessage::Destroy => {
                self.visible = false;
                self.window_id = None;
                WindowMsgHandled::Handled
            }
            WindowMessage::InputFocus => {
                if data1 != 0 {
                    write_input_focus_response(data1, data2, true)
                } else {
                    WindowMsgHandled::Ignored
                }
            }
            WindowMessage::GadgetSelected => {
                let control_id = data1 as WindowId;
                let button_ok_id =
                    NameKeyGenerator::name_to_key("MessageBox.wnd:ButtonOk") as WindowId;
                let button_yes_id =
                    NameKeyGenerator::name_to_key("MessageBox.wnd:ButtonYes") as WindowId;
                let button_no_id =
                    NameKeyGenerator::name_to_key("MessageBox.wnd:ButtonNo") as WindowId;
                let button_cancel_id =
                    NameKeyGenerator::name_to_key("MessageBox.wnd:ButtonCancel") as WindowId;

                if let Some(callbacks) = window.get_user_data::<WindowMessageBoxData>() {
                    if control_id == button_ok_id {
                        callbacks.call_ok();
                        self.result = Some(MessageBoxResult::Ok);
                    } else if control_id == button_yes_id {
                        callbacks.call_yes();
                        self.result = Some(MessageBoxResult::Yes);
                    } else if control_id == button_no_id {
                        callbacks.call_no();
                        self.result = Some(MessageBoxResult::No);
                    } else if control_id == button_cancel_id {
                        callbacks.call_cancel();
                        self.result = Some(MessageBoxResult::Cancel);
                    }
                } else if control_id == button_ok_id {
                    self.result = Some(MessageBoxResult::Ok);
                } else if control_id == button_yes_id {
                    self.result = Some(MessageBoxResult::Yes);
                } else if control_id == button_no_id {
                    self.result = Some(MessageBoxResult::No);
                } else if control_id == button_cancel_id {
                    self.result = Some(MessageBoxResult::Cancel);
                }

                self.visible = false;
                let window_id = window.get_id();
                with_window_manager(|manager| {
                    if let Some(handle) = manager.get_window_by_id(window_id) {
                        let _ = manager.destroy_window(handle);
                        manager.flush_destroy_queue();
                    }
                });

                WindowMsgHandled::Handled
            }
            _ => WindowMsgHandled::Ignored,
        }
    }

    /// Show message box with specified parameters
    pub fn show_message_box(
        &mut self,
        title: &str,
        message: &str,
        message_type: MessageBoxType,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("Showing message box: '{}' - '{}'", title, message);

        self.title = title.to_string();
        self.message = message.to_string();
        self.message_type = message_type;
        self.visible = true;
        self.result = None;

        let button_flags = match self.message_type {
            MessageBoxType::Ok => MSG_BOX_OK,
            MessageBoxType::OkCancel => MSG_BOX_OK | MSG_BOX_CANCEL,
            MessageBoxType::YesNo => MSG_BOX_YES | MSG_BOX_NO,
            MessageBoxType::YesNoCancel => MSG_BOX_YES | MSG_BOX_NO | MSG_BOX_CANCEL,
            MessageBoxType::RetryCancel => MSG_BOX_OK | MSG_BOX_CANCEL,
            MessageBoxType::AbortRetryIgnore => MSG_BOX_OK | MSG_BOX_CANCEL,
        };

        let window = gogo_message_box(
            -1,
            -1,
            -1,
            -1,
            button_flags,
            &self.title,
            &self.message,
            None,
            None,
            None,
            None,
            false,
        );
        self.window_id = window.as_ref().map(|win| win.borrow().get_id());

        Ok(())
    }

    /// Hide message box
    pub fn hide_message_box(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Hiding message box");

        self.visible = false;

        if let Some(window_id) = self.window_id.take() {
            with_window_manager(|manager| {
                if let Some(handle) = manager.get_window_by_id(window_id) {
                    let _ = manager.destroy_window(handle);
                    manager.flush_destroy_queue();
                }
            });
        }

        Ok(())
    }

    /// Set message box result
    pub fn set_result(
        &mut self,
        result: MessageBoxResult,
    ) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Setting message box result: {:?}", result);

        self.result = Some(result);
        self.hide_message_box()?;

        Ok(())
    }

    /// Get message box result (non-blocking)
    pub fn get_result(&self) -> Option<MessageBoxResult> {
        self.result.clone()
    }

    /// Check if message box is visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Get current message
    pub fn get_message(&self) -> &str {
        &self.message
    }

    /// Get current title
    pub fn get_title(&self) -> &str {
        &self.title
    }

    /// Get message box type
    pub fn get_message_type(&self) -> &MessageBoxType {
        &self.message_type
    }
}

impl Default for MessageBoxCallbacks {
    fn default() -> Self {
        Self::new()
    }
}

/// Extended message box with additional features
pub struct ExtendedMessageBoxCallbacks {
    base: MessageBoxCallbacks,
    timeout: Option<u32>,
    default_button: Option<MessageBoxButton>,
    icon_type: String,
}

impl ExtendedMessageBoxCallbacks {
    pub fn new() -> Self {
        Self {
            base: MessageBoxCallbacks::new(),
            timeout: None,
            default_button: None,
            icon_type: "info".to_string(),
        }
    }

    /// Handle extended message box system messages
    pub fn system(
        &mut self,
        window: &GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        debug!(
            "Extended message box system message: {:?} for window: {}",
            msg,
            window.get_name()
        );

        match msg {
            WindowMessage::Destroy => {
                self.base.system(window, msg, data1, data2);
                WindowMsgHandled::Handled
            }
            WindowMessage::InputFocus => {
                if data1 != 0 {
                    write_input_focus_response(data1, data2, true)
                } else {
                    WindowMsgHandled::Ignored
                }
            }
            WindowMessage::GadgetSelected => {
                let control_id = data1 as WindowId;
                let button_ok_id =
                    NameKeyGenerator::name_to_key("MessageBox.wnd:ButtonOk") as WindowId;
                let button_yes_id =
                    NameKeyGenerator::name_to_key("MessageBox.wnd:ButtonYes") as WindowId;
                let button_no_id =
                    NameKeyGenerator::name_to_key("MessageBox.wnd:ButtonNo") as WindowId;
                let button_cancel_id =
                    NameKeyGenerator::name_to_key("MessageBox.wnd:ButtonCancel") as WindowId;

                let mut close = true;
                if let Some(callbacks) = window.get_user_data::<WindowExMessageBoxData>() {
                    let result = if control_id == button_ok_id {
                        self.base.result = Some(MessageBoxResult::Ok);
                        callbacks.call_ok()
                    } else if control_id == button_yes_id {
                        self.base.result = Some(MessageBoxResult::Yes);
                        callbacks.call_yes()
                    } else if control_id == button_no_id {
                        self.base.result = Some(MessageBoxResult::No);
                        callbacks.call_no()
                    } else if control_id == button_cancel_id {
                        self.base.result = Some(MessageBoxResult::Cancel);
                        callbacks.call_cancel()
                    } else {
                        MessageBoxReturnType::Close
                    };
                    close = result == MessageBoxReturnType::Close;
                } else {
                    if control_id == button_ok_id {
                        self.base.result = Some(MessageBoxResult::Ok);
                    } else if control_id == button_yes_id {
                        self.base.result = Some(MessageBoxResult::Yes);
                    } else if control_id == button_no_id {
                        self.base.result = Some(MessageBoxResult::No);
                    } else if control_id == button_cancel_id {
                        self.base.result = Some(MessageBoxResult::Cancel);
                    }
                }

                self.base.visible = !close;
                if close {
                    let window_id = window.get_id();
                    with_window_manager(|manager| {
                        if let Some(handle) = manager.get_window_by_id(window_id) {
                            let _ = manager.destroy_window(handle);
                            manager.flush_destroy_queue();
                        }
                    });
                }

                WindowMsgHandled::Handled
            }
            _ => self.base.system(window, msg, data1, data2),
        }
    }

    /// Show extended message box with timeout and default button
    pub fn show_extended_message_box(
        &mut self,
        title: &str,
        message: &str,
        message_type: MessageBoxType,
        timeout: Option<u32>,
        default_button: Option<MessageBoxButton>,
        icon_type: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Showing extended message box: '{}' with timeout: {:?}, default: {:?}",
            title, timeout, default_button
        );

        self.timeout = timeout;
        self.default_button = default_button;
        self.icon_type = icon_type.to_string();

        self.base.title = title.to_string();
        self.base.message = message.to_string();
        self.base.message_type = message_type;
        self.base.visible = true;
        self.base.result = None;

        let button_flags = match self.base.message_type {
            MessageBoxType::Ok => MSG_BOX_OK,
            MessageBoxType::OkCancel => MSG_BOX_OK | MSG_BOX_CANCEL,
            MessageBoxType::YesNo => MSG_BOX_YES | MSG_BOX_NO,
            MessageBoxType::YesNoCancel => MSG_BOX_YES | MSG_BOX_NO | MSG_BOX_CANCEL,
            MessageBoxType::RetryCancel => MSG_BOX_OK | MSG_BOX_CANCEL,
            MessageBoxType::AbortRetryIgnore => MSG_BOX_OK | MSG_BOX_CANCEL,
        };

        let window = gogo_ex_message_box(
            -1,
            -1,
            -1,
            -1,
            button_flags,
            &self.base.title,
            &self.base.message,
            None,
            None,
            None,
            None,
            None,
        );
        self.base.window_id = window.as_ref().map(|win| win.borrow().get_id());

        Ok(())
    }

    /// Get timeout value
    pub fn get_timeout(&self) -> Option<u32> {
        self.timeout
    }

    /// Get default button
    pub fn get_default_button(&self) -> Option<&MessageBoxButton> {
        self.default_button.as_ref()
    }

    /// Get icon type
    pub fn get_icon_type(&self) -> &str {
        &self.icon_type
    }

    /// Delegate methods to base implementation
    pub fn hide_message_box(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.base.hide_message_box()
    }

    pub fn set_result(
        &mut self,
        result: MessageBoxResult,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.base.set_result(result)
    }

    pub fn get_result(&self) -> Option<MessageBoxResult> {
        self.base.get_result()
    }

    pub fn is_visible(&self) -> bool {
        self.base.is_visible()
    }

    pub fn get_message(&self) -> &str {
        self.base.get_message()
    }

    pub fn get_title(&self) -> &str {
        self.base.get_title()
    }

    pub fn get_message_type(&self) -> &MessageBoxType {
        self.base.get_message_type()
    }
}

impl Default for ExtendedMessageBoxCallbacks {
    fn default() -> Self {
        Self::new()
    }
}

/// Quit message box with special handling
pub struct QuitMessageBoxCallbacks {
    base: MessageBoxCallbacks,
    force_quit: bool,
}

impl QuitMessageBoxCallbacks {
    pub fn new() -> Self {
        Self {
            base: MessageBoxCallbacks::new(),
            force_quit: false,
        }
    }

    /// Handle quit message box system messages
    pub fn system(
        &mut self,
        window: &GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        debug!(
            "Quit message box system message: {:?} for window: {}",
            msg,
            window.get_name()
        );

        match msg {
            WindowMessage::Destroy => {
                self.base.system(window, msg, data1, data2);
                WindowMsgHandled::Handled
            }
            WindowMessage::InputFocus => {
                if data1 != 0 {
                    write_input_focus_response(data1, data2, true)
                } else {
                    WindowMsgHandled::Ignored
                }
            }
            WindowMessage::GadgetSelected => {
                let control_id = data1 as WindowId;
                let button_ok_id =
                    NameKeyGenerator::name_to_key("QuitMessageBox.wnd:ButtonOk") as WindowId;
                let button_yes_id =
                    NameKeyGenerator::name_to_key("QuitMessageBox.wnd:ButtonYes") as WindowId;
                let button_no_id =
                    NameKeyGenerator::name_to_key("QuitMessageBox.wnd:ButtonNo") as WindowId;
                let button_cancel_id =
                    NameKeyGenerator::name_to_key("QuitMessageBox.wnd:ButtonCancel") as WindowId;

                if let Some(callbacks) = window.get_user_data::<WindowMessageBoxData>() {
                    if control_id == button_ok_id {
                        callbacks.call_ok();
                        self.base.result = Some(MessageBoxResult::Ok);
                    } else if control_id == button_yes_id {
                        callbacks.call_yes();
                        self.base.result = Some(MessageBoxResult::Yes);
                    } else if control_id == button_no_id {
                        callbacks.call_no();
                        self.base.result = Some(MessageBoxResult::No);
                    } else if control_id == button_cancel_id {
                        callbacks.call_cancel();
                        self.base.result = Some(MessageBoxResult::Cancel);
                    }
                } else if control_id == button_ok_id {
                    self.base.result = Some(MessageBoxResult::Ok);
                } else if control_id == button_yes_id {
                    self.base.result = Some(MessageBoxResult::Yes);
                } else if control_id == button_no_id {
                    self.base.result = Some(MessageBoxResult::No);
                } else if control_id == button_cancel_id {
                    self.base.result = Some(MessageBoxResult::Cancel);
                }

                self.base.visible = false;
                let window_id = window.get_id();
                with_window_manager(|manager| {
                    if let Some(handle) = manager.get_window_by_id(window_id) {
                        let _ = manager.destroy_window(handle);
                        manager.flush_destroy_queue();
                    }
                });

                WindowMsgHandled::Handled
            }
            _ => WindowMsgHandled::Ignored,
        }
    }

    /// Show quit confirmation dialog
    pub fn show_quit_dialog(&mut self, force_quit: bool) -> Result<(), Box<dyn std::error::Error>> {
        info!("Showing quit dialog (force: {})", force_quit);

        self.force_quit = force_quit;

        let message = if force_quit {
            "Are you sure you want to quit immediately?"
        } else {
            "Are you sure you want to quit? Any unsaved progress will be lost."
        };

        let window = quit_message_box_yes_no("Quit Game", message, None, None);
        self.base.visible = true;
        self.base.title = "Quit Game".to_string();
        self.base.message = message.to_string();
        self.base.message_type = MessageBoxType::YesNo;
        self.base.window_id = window.as_ref().map(|win| win.borrow().get_id());

        Ok(())
    }

    /// Process quit result
    pub fn process_quit_result(&mut self) -> Result<Option<bool>, Box<dyn std::error::Error>> {
        match self.base.get_result() {
            Some(MessageBoxResult::Yes) => {
                info!("User confirmed quit");
                Ok(Some(true))
            }
            Some(MessageBoxResult::No) => {
                info!("User cancelled quit");
                Ok(Some(false))
            }
            Some(_) => {
                warn!("Unexpected result from quit dialog");
                Ok(Some(false))
            }
            None => {
                // No result yet
                Ok(None)
            }
        }
    }

    /// Check if this is a force quit
    pub fn is_force_quit(&self) -> bool {
        self.force_quit
    }

    /// Delegate methods to base implementation
    pub fn hide_message_box(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.base.hide_message_box()
    }

    pub fn set_result(
        &mut self,
        result: MessageBoxResult,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.base.set_result(result)
    }

    pub fn get_result(&self) -> Option<MessageBoxResult> {
        self.base.get_result()
    }

    pub fn is_visible(&self) -> bool {
        self.base.is_visible()
    }
}

impl Default for QuitMessageBoxCallbacks {
    fn default() -> Self {
        Self::new()
    }
}

fn resize_message_box_children(parent: &Rc<RefCell<GameWindow>>, width: i32, height: i32) {
    let (current_width, current_height) = parent.borrow().get_size();
    if current_width <= 0 || current_height <= 0 {
        return;
    }
    let ratio_x = width as f32 / current_width as f32;
    let ratio_y = height as f32 / current_height as f32;
    let _ = parent.borrow_mut().set_size(width, height);

    let children = parent.borrow().children().to_vec();
    for child in children {
        let (child_width, child_height) = child.borrow().get_size();
        let new_width = (child_width as f32 * ratio_x) as i32;
        let new_height = (child_height as f32 * ratio_y) as i32;
        let _ = child.borrow_mut().set_size(new_width, new_height);

        let (child_x, child_y) = child.borrow().get_position();
        let new_x = (child_x as f32 * ratio_x) as i32;
        let new_y = (child_y as f32 * ratio_y) as i32;
        let _ = child.borrow_mut().set_position(new_x, new_y);
    }
}

fn lookup_window_by_name(
    windows: &[Rc<RefCell<GameWindow>>],
    name: &str,
) -> Option<Rc<RefCell<GameWindow>>> {
    windows
        .iter()
        .find(|window| window.borrow().get_name().eq_ignore_ascii_case(name))
        .cloned()
}

fn gogo_message_box(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    button_flags: u16,
    title: &str,
    body: &str,
    yes_callback: Option<MessageBoxFunc>,
    no_callback: Option<MessageBoxFunc>,
    ok_callback: Option<MessageBoxFunc>,
    cancel_callback: Option<MessageBoxFunc>,
    use_logo: bool,
) -> Option<Rc<RefCell<GameWindow>>> {
    if button_flags == 0 {
        return None;
    }

    with_window_manager(|manager| {
        let script = if use_logo {
            "Menus/QuitMessageBox.wnd"
        } else {
            "Menus/MessageBox.wnd"
        };
        let info = manager.create_windows_from_script(script).ok()?;
        let menu_name = if use_logo {
            "QuitMessageBox.wnd:"
        } else {
            "MessageBox.wnd:"
        };

        let true_parent = lookup_window_by_name(&info.windows, menu_name).or_else(|| {
            info.windows
                .iter()
                .find(|w| w.borrow().get_parent().is_none())
                .cloned()
        })?;

        let parent_id =
            NameKeyGenerator::name_to_key(&format!("{}MessageBoxParent", menu_name)) as WindowId;
        let parent = manager
            .get_window_by_id(parent_id)
            .unwrap_or_else(|| true_parent.clone());

        let _ = manager.set_modal(true_parent.clone());
        let _ = manager.set_focus(None);
        let _ = manager.set_focus(Some(&parent));

        if width > 0 && height > 0 {
            resize_message_box_children(&parent, width, height);
        }
        if x >= 0 && y >= 0 {
            let _ = parent.borrow_mut().set_position(x, y);
        }

        let button_ok_id =
            NameKeyGenerator::name_to_key(&format!("{}ButtonOk", menu_name)) as WindowId;
        let button_yes_id =
            NameKeyGenerator::name_to_key(&format!("{}ButtonYes", menu_name)) as WindowId;
        let button_no_id =
            NameKeyGenerator::name_to_key(&format!("{}ButtonNo", menu_name)) as WindowId;
        let button_cancel_id =
            NameKeyGenerator::name_to_key(&format!("{}ButtonCancel", menu_name)) as WindowId;

        let button_ok = manager.get_window_by_id(button_ok_id);
        let button_yes = manager.get_window_by_id(button_yes_id);
        let button_no = manager.get_window_by_id(button_no_id);
        let button_cancel = manager.get_window_by_id(button_cancel_id);

        let mut button_x = [0; 3];
        let mut button_y = [0; 3];
        if let Some(ref ok) = button_ok {
            let (x, y) = ok.borrow().get_position();
            button_x[0] = x;
            button_y[0] = y;
        }
        if let Some(ref no) = button_no {
            let (x, y) = no.borrow().get_position();
            button_x[1] = x;
            button_y[1] = y;
        }
        if let Some(ref cancel) = button_cancel {
            let (x, y) = cancel.borrow().get_position();
            button_x[2] = x;
            button_y[2] = y;
        }

        if (button_flags & (MSG_BOX_OK | MSG_BOX_YES)) == (MSG_BOX_OK | MSG_BOX_YES) {
            warn!("Message box has both OK and YES buttons set.");
        }

        if (button_flags & MSG_BOX_OK) == MSG_BOX_OK {
            if let Some(ref ok) = button_ok {
                let _ = ok.borrow_mut().set_position(button_x[0], button_y[0]);
                let _ = ok.borrow_mut().show();
            }
        } else if (button_flags & MSG_BOX_YES) == MSG_BOX_YES {
            if let Some(ref yes) = button_yes {
                let _ = yes.borrow_mut().set_position(button_x[0], button_y[0]);
                let _ = yes.borrow_mut().show();
            }
        }

        if (button_flags & (MSG_BOX_NO | MSG_BOX_CANCEL)) == (MSG_BOX_NO | MSG_BOX_CANCEL) {
            if let Some(ref no) = button_no {
                let _ = no.borrow_mut().set_position(button_x[1], button_y[1]);
                let _ = no.borrow_mut().show();
            }
            if let Some(ref cancel) = button_cancel {
                let _ = cancel.borrow_mut().set_position(button_x[2], button_y[2]);
                let _ = cancel.borrow_mut().show();
            }
        } else if (button_flags & MSG_BOX_NO) == MSG_BOX_NO {
            if let Some(ref no) = button_no {
                let _ = no.borrow_mut().set_position(button_x[2], button_y[2]);
                let _ = no.borrow_mut().show();
            }
        } else if (button_flags & MSG_BOX_CANCEL) == MSG_BOX_CANCEL {
            if let Some(ref cancel) = button_cancel {
                let _ = cancel.borrow_mut().set_position(button_x[2], button_y[2]);
                let _ = cancel.borrow_mut().show();
            }
        }

        let title_id =
            NameKeyGenerator::name_to_key(&format!("{}StaticTextTitle", menu_name)) as WindowId;
        if let Some(title_window) = manager.get_window_by_id(title_id) {
            let _ = title_window.borrow_mut().set_text(title);
        }
        let message_id =
            NameKeyGenerator::name_to_key(&format!("{}StaticTextMessage", menu_name)) as WindowId;
        if let Some(message_window) = manager.get_window_by_id(message_id) {
            let _ = message_window.borrow_mut().set_text(body);
        }

        true_parent
            .borrow_mut()
            .set_user_data(WindowMessageBoxData::new(
                yes_callback,
                no_callback,
                ok_callback,
                cancel_callback,
            ));

        let _ = parent.borrow_mut().show();
        let _ = parent.borrow_mut().bring_to_front();

        Some(true_parent)
    })
}

fn gogo_ex_message_box(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    button_flags: u16,
    title: &str,
    body: &str,
    yes_callback: Option<ExtendedMessageBoxFunc>,
    no_callback: Option<ExtendedMessageBoxFunc>,
    ok_callback: Option<ExtendedMessageBoxFunc>,
    cancel_callback: Option<ExtendedMessageBoxFunc>,
    user_data: Option<Box<dyn Any>>,
) -> Option<Rc<RefCell<GameWindow>>> {
    if button_flags == 0 {
        return None;
    }

    with_window_manager(|manager| {
        let info = manager
            .create_windows_from_script("Menus/MessageBox.wnd")
            .ok()?;
        let menu_name = "MessageBox.wnd:";
        let parent = lookup_window_by_name(&info.windows, menu_name).or_else(|| {
            info.windows
                .iter()
                .find(|w| w.borrow().get_parent().is_none())
                .cloned()
        })?;

        let _ = manager.set_modal(parent.clone());
        let _ = manager.set_focus(None);
        let _ = manager.set_focus(Some(&parent));

        if width > 0 && height > 0 {
            resize_message_box_children(&parent, width, height);
        }
        if x >= 0 && y >= 0 {
            let _ = parent.borrow_mut().set_position(x, y);
        }

        let button_ok_id = NameKeyGenerator::name_to_key("MessageBox.wnd:ButtonOk") as WindowId;
        let button_yes_id = NameKeyGenerator::name_to_key("MessageBox.wnd:ButtonYes") as WindowId;
        let button_no_id = NameKeyGenerator::name_to_key("MessageBox.wnd:ButtonNo") as WindowId;
        let button_cancel_id =
            NameKeyGenerator::name_to_key("MessageBox.wnd:ButtonCancel") as WindowId;

        let button_ok = manager.get_window_by_id(button_ok_id);
        let button_yes = manager.get_window_by_id(button_yes_id);
        let button_no = manager.get_window_by_id(button_no_id);
        let button_cancel = manager.get_window_by_id(button_cancel_id);

        let mut button_x = [0; 3];
        let mut button_y = [0; 3];
        if let Some(ref ok) = button_ok {
            let (x, y) = ok.borrow().get_position();
            button_x[0] = x;
            button_y[0] = y;
        }
        if let Some(ref no) = button_no {
            let (x, y) = no.borrow().get_position();
            button_x[1] = x;
            button_y[1] = y;
        }
        if let Some(ref cancel) = button_cancel {
            let (x, y) = cancel.borrow().get_position();
            button_x[2] = x;
            button_y[2] = y;
        }

        if (button_flags & (MSG_BOX_OK | MSG_BOX_YES)) == (MSG_BOX_OK | MSG_BOX_YES) {
            warn!("Message box has both OK and YES buttons set.");
        }

        if (button_flags & MSG_BOX_OK) == MSG_BOX_OK {
            if let Some(ref ok) = button_ok {
                let _ = ok.borrow_mut().set_position(button_x[0], button_y[0]);
                let _ = ok.borrow_mut().show();
            }
        } else if (button_flags & MSG_BOX_YES) == MSG_BOX_YES {
            if let Some(ref yes) = button_yes {
                let _ = yes.borrow_mut().set_position(button_x[0], button_y[0]);
                let _ = yes.borrow_mut().show();
            }
        }

        if (button_flags & (MSG_BOX_NO | MSG_BOX_CANCEL)) == (MSG_BOX_NO | MSG_BOX_CANCEL) {
            if let Some(ref no) = button_no {
                let _ = no.borrow_mut().set_position(button_x[1], button_y[1]);
                let _ = no.borrow_mut().show();
            }
            if let Some(ref cancel) = button_cancel {
                let _ = cancel.borrow_mut().set_position(button_x[2], button_y[2]);
                let _ = cancel.borrow_mut().show();
            }
        } else if (button_flags & MSG_BOX_NO) == MSG_BOX_NO {
            if let Some(ref no) = button_no {
                let _ = no.borrow_mut().set_position(button_x[2], button_y[2]);
                let _ = no.borrow_mut().show();
            }
        } else if (button_flags & MSG_BOX_CANCEL) == MSG_BOX_CANCEL {
            if let Some(ref cancel) = button_cancel {
                let _ = cancel.borrow_mut().set_position(button_x[2], button_y[2]);
                let _ = cancel.borrow_mut().show();
            }
        }

        let title_id = NameKeyGenerator::name_to_key("MessageBox.wnd:StaticTextTitle") as WindowId;
        if let Some(title_window) = manager.get_window_by_id(title_id) {
            let _ = title_window.borrow_mut().set_text(title);
        }
        let message_id =
            NameKeyGenerator::name_to_key("MessageBox.wnd:StaticTextMessage") as WindowId;
        if let Some(message_window) = manager.get_window_by_id(message_id) {
            let _ = message_window.borrow_mut().set_text(body);
        }

        parent
            .borrow_mut()
            .set_user_data(WindowExMessageBoxData::new(
                yes_callback,
                no_callback,
                ok_callback,
                cancel_callback,
                user_data,
            ));
        parent
            .borrow_mut()
            .set_system_callback(|window, msg, data1, data2| {
                let system = get_message_box_system();
                let system = system.read().unwrap_or_else(|e| e.into_inner());
                let extended = system.get_extended().clone();
                let mut extended_guard = extended.write().unwrap_or_else(|e| e.into_inner());
                extended_guard.system(window, msg, data1, data2)
            });

        let _ = parent.borrow_mut().show();
        let _ = parent.borrow_mut().bring_to_front();

        Some(parent)
    })
}

pub fn message_box_yes_no(
    title: &str,
    body: &str,
    yes_callback: Option<MessageBoxFunc>,
    no_callback: Option<MessageBoxFunc>,
) -> Option<Rc<RefCell<GameWindow>>> {
    gogo_message_box(
        -1,
        -1,
        -1,
        -1,
        MSG_BOX_YES | MSG_BOX_NO,
        title,
        body,
        yes_callback,
        no_callback,
        None,
        None,
        false,
    )
}

pub fn quit_message_box_yes_no(
    title: &str,
    body: &str,
    yes_callback: Option<MessageBoxFunc>,
    no_callback: Option<MessageBoxFunc>,
) -> Option<Rc<RefCell<GameWindow>>> {
    gogo_message_box(
        -1,
        -1,
        -1,
        -1,
        MSG_BOX_YES | MSG_BOX_NO,
        title,
        body,
        yes_callback,
        no_callback,
        None,
        None,
        true,
    )
}

pub fn message_box_yes_no_cancel(
    title: &str,
    body: &str,
    yes_callback: Option<MessageBoxFunc>,
    no_callback: Option<MessageBoxFunc>,
    cancel_callback: Option<MessageBoxFunc>,
) -> Option<Rc<RefCell<GameWindow>>> {
    gogo_message_box(
        -1,
        -1,
        -1,
        -1,
        MSG_BOX_YES | MSG_BOX_NO | MSG_BOX_CANCEL,
        title,
        body,
        yes_callback,
        no_callback,
        None,
        cancel_callback,
        false,
    )
}

pub fn message_box_ok_cancel(
    title: &str,
    body: &str,
    ok_callback: Option<MessageBoxFunc>,
    cancel_callback: Option<MessageBoxFunc>,
) -> Option<Rc<RefCell<GameWindow>>> {
    gogo_message_box(
        -1,
        -1,
        -1,
        -1,
        MSG_BOX_OK | MSG_BOX_CANCEL,
        title,
        body,
        None,
        None,
        ok_callback,
        cancel_callback,
        false,
    )
}

pub fn message_box_ok(
    title: &str,
    body: &str,
    ok_callback: Option<MessageBoxFunc>,
) -> Option<Rc<RefCell<GameWindow>>> {
    gogo_message_box(
        -1,
        -1,
        -1,
        -1,
        MSG_BOX_OK,
        title,
        body,
        None,
        None,
        ok_callback,
        None,
        false,
    )
}

pub fn message_box_cancel(
    title: &str,
    body: &str,
    cancel_callback: Option<MessageBoxFunc>,
) -> Option<Rc<RefCell<GameWindow>>> {
    gogo_message_box(
        -1,
        -1,
        -1,
        -1,
        MSG_BOX_CANCEL,
        title,
        body,
        None,
        None,
        None,
        cancel_callback,
        false,
    )
}

pub fn ex_message_box_yes_no(
    title: &str,
    body: &str,
    user_data: Option<Box<dyn Any>>,
    yes_callback: Option<ExtendedMessageBoxFunc>,
    no_callback: Option<ExtendedMessageBoxFunc>,
) -> Option<Rc<RefCell<GameWindow>>> {
    gogo_ex_message_box(
        -1,
        -1,
        -1,
        -1,
        MSG_BOX_YES | MSG_BOX_NO,
        title,
        body,
        yes_callback,
        no_callback,
        None,
        None,
        user_data,
    )
}

pub fn ex_message_box_yes_no_cancel(
    title: &str,
    body: &str,
    user_data: Option<Box<dyn Any>>,
    yes_callback: Option<ExtendedMessageBoxFunc>,
    no_callback: Option<ExtendedMessageBoxFunc>,
    cancel_callback: Option<ExtendedMessageBoxFunc>,
) -> Option<Rc<RefCell<GameWindow>>> {
    gogo_ex_message_box(
        -1,
        -1,
        -1,
        -1,
        MSG_BOX_YES | MSG_BOX_NO | MSG_BOX_CANCEL,
        title,
        body,
        yes_callback,
        no_callback,
        None,
        cancel_callback,
        user_data,
    )
}

pub fn ex_message_box_ok_cancel(
    title: &str,
    body: &str,
    user_data: Option<Box<dyn Any>>,
    ok_callback: Option<ExtendedMessageBoxFunc>,
    cancel_callback: Option<ExtendedMessageBoxFunc>,
) -> Option<Rc<RefCell<GameWindow>>> {
    gogo_ex_message_box(
        -1,
        -1,
        -1,
        -1,
        MSG_BOX_OK | MSG_BOX_CANCEL,
        title,
        body,
        None,
        None,
        ok_callback,
        cancel_callback,
        user_data,
    )
}

pub fn ex_message_box_ok(
    title: &str,
    body: &str,
    user_data: Option<Box<dyn Any>>,
    ok_callback: Option<ExtendedMessageBoxFunc>,
) -> Option<Rc<RefCell<GameWindow>>> {
    gogo_ex_message_box(
        -1,
        -1,
        -1,
        -1,
        MSG_BOX_OK,
        title,
        body,
        None,
        None,
        ok_callback,
        None,
        user_data,
    )
}

pub fn ex_message_box_cancel(
    title: &str,
    body: &str,
    user_data: Option<Box<dyn Any>>,
    cancel_callback: Option<ExtendedMessageBoxFunc>,
) -> Option<Rc<RefCell<GameWindow>>> {
    gogo_ex_message_box(
        -1,
        -1,
        -1,
        -1,
        MSG_BOX_CANCEL,
        title,
        body,
        None,
        None,
        None,
        cancel_callback,
        user_data,
    )
}

/// Combined message box system
pub struct MessageBoxSystem {
    standard: Arc<RwLock<MessageBoxCallbacks>>,
    extended: Arc<RwLock<ExtendedMessageBoxCallbacks>>,
    quit: Arc<RwLock<QuitMessageBoxCallbacks>>,
}

impl MessageBoxSystem {
    pub fn new() -> Self {
        Self {
            standard: Arc::new(RwLock::new(MessageBoxCallbacks::new())),
            extended: Arc::new(RwLock::new(ExtendedMessageBoxCallbacks::new())),
            quit: Arc::new(RwLock::new(QuitMessageBoxCallbacks::new())),
        }
    }

    pub fn get_standard(&self) -> Arc<RwLock<MessageBoxCallbacks>> {
        self.standard.clone()
    }

    pub fn get_extended(&self) -> Arc<RwLock<ExtendedMessageBoxCallbacks>> {
        self.extended.clone()
    }

    pub fn get_quit(&self) -> Arc<RwLock<QuitMessageBoxCallbacks>> {
        self.quit.clone()
    }

    /// Show standard message box through the system
    pub fn show_message_box(
        &self,
        title: &str,
        message: &str,
        message_type: MessageBoxType,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut standard = self.standard.write().unwrap_or_else(|e| e.into_inner());
        standard.show_message_box(title, message, message_type)
    }

    /// Show quit dialog through the system
    pub fn show_quit_dialog(&self, force_quit: bool) -> Result<(), Box<dyn std::error::Error>> {
        let mut quit = self.quit.write().unwrap_or_else(|e| e.into_inner());
        quit.show_quit_dialog(force_quit)
    }
}

impl Default for MessageBoxSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Global message box system instance
lazy_static::lazy_static! {
    pub static ref THE_MESSAGE_BOX_SYSTEM: Arc<RwLock<MessageBoxSystem>> =
        Arc::new(RwLock::new(MessageBoxSystem::new()));
}

/// Helper function to get the global message box system
pub fn get_message_box_system() -> Arc<RwLock<MessageBoxSystem>> {
    THE_MESSAGE_BOX_SYSTEM.clone()
}

/// Convenience functions for global message box operations
pub fn show_message_box(
    title: &str,
    message: &str,
    message_type: MessageBoxType,
) -> Result<(), Box<dyn std::error::Error>> {
    let system = get_message_box_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    system.show_message_box(title, message, message_type)
}

pub fn show_quit_dialog(force_quit: bool) -> Result<(), Box<dyn std::error::Error>> {
    let system = get_message_box_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    system.show_quit_dialog(force_quit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_box_callbacks() {
        let mut message_box = MessageBoxCallbacks::new();

        // Test initial state
        assert!(!message_box.is_visible());
        assert!(message_box.get_result().is_none());

        // Test showing message box
        assert!(message_box
            .show_message_box("Test", "This is a test", MessageBoxType::YesNo)
            .is_ok());
        assert!(message_box.is_visible());
        assert_eq!(message_box.get_title(), "Test");
        assert_eq!(message_box.get_message(), "This is a test");
        assert_eq!(message_box.get_message_type(), &MessageBoxType::YesNo);

        // Test setting result
        assert!(message_box.set_result(MessageBoxResult::Yes).is_ok());
        assert!(!message_box.is_visible());
        assert_eq!(message_box.get_result(), Some(MessageBoxResult::Yes));
    }

    #[test]
    fn test_extended_message_box() {
        let mut extended = ExtendedMessageBoxCallbacks::new();

        // Test showing extended message box
        assert!(extended
            .show_extended_message_box(
                "Extended Test",
                "This is an extended test",
                MessageBoxType::OkCancel,
                Some(30),
                Some(MessageBoxButton::Ok),
                "warning"
            )
            .is_ok());

        assert!(extended.is_visible());
        assert_eq!(extended.get_timeout(), Some(30));
        assert_eq!(extended.get_default_button(), Some(&MessageBoxButton::Ok));
        assert_eq!(extended.get_icon_type(), "warning");
    }

    #[test]
    fn test_quit_message_box() {
        let mut quit_box = QuitMessageBoxCallbacks::new();

        // Test showing quit dialog
        assert!(quit_box.show_quit_dialog(false).is_ok());
        assert!(quit_box.is_visible());
        assert!(!quit_box.is_force_quit());

        // Test force quit
        assert!(quit_box.show_quit_dialog(true).is_ok());
        assert!(quit_box.is_force_quit());

        // Test result processing
        assert!(quit_box.set_result(MessageBoxResult::Yes).is_ok());
        let result = quit_box.process_quit_result().unwrap();
        assert_eq!(result, Some(true));
    }

    #[test]
    fn test_message_box_system() {
        let system = MessageBoxSystem::new();

        // Test that all components are accessible
        assert!(system.get_standard().read().is_ok());
        assert!(system.get_extended().read().is_ok());
        assert!(system.get_quit().read().is_ok());

        // Test system-level operations
        assert!(system
            .show_message_box("System Test", "Testing", MessageBoxType::Ok)
            .is_ok());
        assert!(system.show_quit_dialog(false).is_ok());
    }

    #[test]
    fn test_global_functions() {
        assert!(show_message_box(
            "Global Test",
            "Testing global functions",
            MessageBoxType::Ok
        )
        .is_ok());
        assert!(show_quit_dialog(true).is_ok());
    }

    #[test]
    fn test_message_box_types() {
        use MessageBoxType::*;

        // Test all message box types can be created
        let types = vec![
            Ok,
            OkCancel,
            YesNo,
            YesNoCancel,
            RetryCancel,
            AbortRetryIgnore,
        ];

        for msg_type in types {
            let mut message_box = MessageBoxCallbacks::new();
            assert!(message_box
                .show_message_box("Test", "Test", msg_type)
                .is_ok());
        }
    }

    #[test]
    fn test_message_box_results() {
        use MessageBoxResult::*;

        // Test all message box results can be set
        let results = vec![Ok, Cancel, Yes, No, Retry, Abort, Ignore, Closed];

        for result in results {
            let mut message_box = MessageBoxCallbacks::new();
            message_box
                .show_message_box("Test", "Test", MessageBoxType::Ok)
                .unwrap();
            assert!(message_box.set_result(result.clone()).is_ok());
            assert_eq!(message_box.get_result(), Some(result));
        }
    }
}
