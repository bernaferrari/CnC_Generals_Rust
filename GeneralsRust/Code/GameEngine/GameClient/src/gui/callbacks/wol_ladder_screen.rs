//! WOLLadderScreen.cpp callback port.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};

use crate::gui::{
    get_shell, with_window_manager, GameWindow, WindowLayout, WindowMessage, WindowMsgData,
    WindowMsgHandled,
};
use game_engine::common::ascii_string::AsciiString;
use game_engine::common::game_engine::get_game_engine;
use game_engine::common::ini::ini_webpage_url::IniWebpageUrl;
use game_engine::common::name_key_generator::NameKeyGenerator;

const KEY_ESC: usize = 0x1B;
const KEY_STATE_UP: usize = 0x0001;

#[derive(Default)]
struct WolLadderState {
    parent_id: u32,
    button_back_id: u32,
    window_ladder_id: u32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    button_back: Option<Rc<RefCell<GameWindow>>>,
    window_ladder: Option<Rc<RefCell<GameWindow>>>,
}

static WOL_LADDER_STATE: OnceLock<Mutex<WolLadderState>> = OnceLock::new();
static WEBPAGES_LOADED: OnceLock<Mutex<bool>> = OnceLock::new();

fn wol_ladder_state() -> &'static Mutex<WolLadderState> {
    WOL_LADDER_STATE.get_or_init(|| Mutex::new(WolLadderState::default()))
}

fn name_to_id(name: &str) -> u32 {
    NameKeyGenerator::name_to_key(name) as u32
}

fn locate_webpages_ini() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(engine) = get_game_engine().and_then(|engine| engine.lock().ok()) {
        for base in engine.data_paths() {
            candidates.push(PathBuf::from(base).join("INI").join("Webpages.ini"));
            candidates.push(
                PathBuf::from(base)
                    .join("INI")
                    .join("Default")
                    .join("Webpages.ini"),
            );
        }
    }

    for path in candidates {
        if path.exists() {
            return Some(path);
        }
    }
    None
}

fn ensure_webpages_loaded() -> bool {
    let mut loaded_guard = WEBPAGES_LOADED
        .get_or_init(|| Mutex::new(false))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if *loaded_guard {
        return true;
    }

    if let Some(path) = locate_webpages_ini() {
        if IniWebpageUrl::load_webpage_urls_from_file(&path).is_ok() {
            *loaded_guard = true;
            return true;
        }
    }

    false
}

pub fn wol_ladder_screen_init(layout: &WindowLayout, _user_data: Option<&mut dyn std::any::Any>) {
    get_shell().show_shell_map(true);

    let parent_id = name_to_id("WOLLadderScreen.wnd:LadderParent");
    let button_back_id = name_to_id("WOLLadderScreen.wnd:ButtonBack");
    let window_ladder_id = name_to_id("WOLLadderScreen.wnd:WindowLadder");

    let parent = with_window_manager(|manager| manager.get_window_by_id(parent_id as i32));
    let button_back =
        with_window_manager(|manager| manager.get_window_by_id(button_back_id as i32));
    let window_ladder =
        with_window_manager(|manager| manager.get_window_by_id(window_ladder_id as i32));

    if ensure_webpages_loaded() {
        let tag = AsciiString::from("MessageBoard");
        let _ = IniWebpageUrl::open_webpage_url_external(&tag);
    }

    layout.hide(false);

    if let Some(parent) = parent.as_ref() {
        let _ = with_window_manager(|manager| manager.set_focus(Some(parent)));
    }

    let mut state = wol_ladder_state().lock().unwrap_or_else(|e| e.into_inner());
    state.parent_id = parent_id;
    state.button_back_id = button_back_id;
    state.window_ladder_id = window_ladder_id;
    state.parent = parent;
    state.button_back = button_back;
    state.window_ladder = window_ladder;
}

pub fn wol_ladder_screen_shutdown(
    layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
    // TODO: C++ calls TheWebBrowser->closeBrowserWindow() here to close any
    // open browser window from the ladder/message board page.
    layout.hide(true);
    get_shell().shutdown_complete(layout);
}

pub fn wol_ladder_screen_update(
    _layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
}

pub fn wol_ladder_screen_input(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    if msg != WindowMessage::Char || data1 != KEY_ESC {
        return WindowMsgHandled::Ignored;
    }
    if (data2 & KEY_STATE_UP) == 0 {
        return WindowMsgHandled::Handled;
    }

    let state = wol_ladder_state().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(parent) = state.parent.as_ref() {
        let _ = parent.borrow_mut().send_system_message(
            WindowMessage::GadgetSelected,
            state.button_back_id,
            state.button_back_id,
        );
    }

    WindowMsgHandled::Handled
}

pub fn wol_ladder_screen_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::Create => WindowMsgHandled::Handled,
        WindowMessage::Destroy => WindowMsgHandled::Handled,
        WindowMessage::InputFocus => {
            // TODO: C++ writes *(Bool*)mData2 = TRUE when mData1 != 0 to accept focus
            WindowMsgHandled::Handled
        }
        WindowMessage::GadgetSelected => {
            let control_id = data1 as u32;
            let state = wol_ladder_state().lock().unwrap_or_else(|e| e.into_inner());
            if control_id == state.button_back_id {
                let _ = get_shell().pop();
                return WindowMsgHandled::Handled;
            }
            WindowMsgHandled::Handled
        }
        WindowMessage::EditDone => WindowMsgHandled::Handled,
        _ => WindowMsgHandled::Ignored,
    }
}
