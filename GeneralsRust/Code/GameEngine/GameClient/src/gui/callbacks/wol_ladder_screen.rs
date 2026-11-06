//! WOLLadderScreen.cpp callback port.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crate::gui::callbacks::online_callback_support::dispatch_esc_gadget_selected;
use crate::gui::{
    GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled, get_shell,
    write_input_focus_response,
};
use crate::w3d_web_browser::W3DWebBrowser;
use game_engine::common::ascii_string::AsciiString;
use game_engine::common::game_engine::get_game_engine;
use game_engine::common::ini::ini_webpage_url::IniWebpageUrl;
use game_engine::common::name_key_generator::NameKeyGenerator;

const KEY_ESC: usize = 0x1B;
const KEY_STATE_UP: usize = 0x0001;

#[derive(Default)]
pub(crate) struct WolLadderState {
    parent_id: i32,
    button_back_id: i32,
    window_ladder_id: i32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    button_back: Option<Rc<RefCell<GameWindow>>>,
    window_ladder: Option<Rc<RefCell<GameWindow>>>,
}

#[cfg(test)]
thread_local! {
    static TEST_WOL_LADDER_STATE: std::cell::OnceCell<Rc<RefCell<WolLadderState>>> =
        const { std::cell::OnceCell::new() };
    static TEST_LADDER_BROWSER: RefCell<W3DWebBrowser> = RefCell::new(W3DWebBrowser::new());
    static TEST_WEBPAGES_LOADED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn wol_ladder_state_on_owner(
    owner: Option<&crate::gui::window_manager::WindowManagerOwner>,
) -> Rc<RefCell<WolLadderState>> {
    if let Some(owner) = owner {
        return owner.wol_ladder_state_handle();
    }
    if let Some(handle) = crate::core::game_client::with_live_game_client_mut(|client| {
        client.window_manager_owner_ref().wol_ladder_state_handle()
    }) {
        return handle;
    }
    #[cfg(test)]
    {
        return TEST_WOL_LADDER_STATE.with(|cell| {
            cell.get_or_init(|| Rc::new(RefCell::new(WolLadderState::default())))
                .clone()
        });
    }
    #[cfg(not(test))]
    {
        Rc::new(RefCell::new(WolLadderState::default()))
    }
}

fn wol_ladder_state() -> Rc<RefCell<WolLadderState>> {
    wol_ladder_state_on_owner(None)
}

fn with_ladder_browser_mut<R>(f: impl FnOnce(&mut W3DWebBrowser) -> R) -> R {
    let mut f = Some(f);
    if let Some(result) = crate::core::game_client::with_live_game_client_mut(|client| {
        client
            .window_manager_owner_ref()
            .with_wol_ladder_browser_mut(|browser| f.take().expect("ladder browser")(browser))
    }) {
        return result;
    }
    #[cfg(test)]
    {
        return TEST_LADDER_BROWSER.with(|slot| f.take().expect("ladder browser")(&mut slot.borrow_mut()));
    }
    #[cfg(not(test))]
    {
        f.take().expect("ladder browser")(&mut W3DWebBrowser::new())
    }
}

fn webpages_loaded() -> bool {
    if let Some(loaded) = crate::core::game_client::with_live_game_client_mut(|client| {
        client.window_manager_owner_ref().wol_ladder_webpages_loaded()
    }) {
        return loaded;
    }
    #[cfg(test)]
    {
        return TEST_WEBPAGES_LOADED.with(|c| c.get());
    }
    #[cfg(not(test))]
    {
        false
    }
}

fn set_webpages_loaded(loaded: bool) {
    if let Some(()) = crate::core::game_client::with_live_game_client_mut(|client| {
        client
            .window_manager_owner_ref()
            .set_wol_ladder_webpages_loaded(loaded);
    }) {
        return;
    }
    #[cfg(test)]
    {
        TEST_WEBPAGES_LOADED.with(|c| c.set(loaded));
        return;
    }
    #[cfg(not(test))]
    {
        let _ = loaded;
    }
}

fn name_to_id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(name) as i32
}

fn locate_webpages_ini() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(engine) = get_game_engine() {
        let guard = engine.lock();
        for base in guard.data_paths() {
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
    if webpages_loaded() {
        return true;
    }

    if let Some(path) = locate_webpages_ini() {
        if IniWebpageUrl::load_webpage_urls_from_file(&path).is_ok() {
            set_webpages_loaded(true);
            return true;
        }
    }

    false
}

pub fn wol_ladder_screen_init(layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    get_shell().show_shell_map(true);

    let parent_id = name_to_id("WOLLadderScreen.wnd:LadderParent");
    let button_back_id = name_to_id("WOLLadderScreen.wnd:ButtonBack");
    let window_ladder_id = name_to_id("WOLLadderScreen.wnd:WindowLadder");

    let (parent, button_back, window_ladder) =
        crate::core::game_client::with_live_game_client_mut(|client| {
            let owner = client.window_manager_owner_ref();
            (
                owner.with_manager(|manager| manager.get_window_by_id(parent_id as i32)),
                owner.with_manager(|manager| manager.get_window_by_id(button_back_id as i32)),
                owner.with_manager(|manager| manager.get_window_by_id(window_ladder_id as i32)),
            )
        })
        .unwrap_or((None, None, None));

    if ensure_webpages_loaded() {
        if let Some(window) = window_ladder.as_ref() {
            with_ladder_browser_mut(|browser| {
                let _ = browser
                    .borrow_mut()
                    .create_browser_window("MessageBoard", &window.borrow());
            });
        }
    }

    layout.hide(false);

    if let Some(parent) = parent.as_ref() {
        let _ = crate::core::game_client::with_live_game_client_mut(|client| {
            client
                .window_manager_owner_ref()
                .with_manager(|manager| manager.set_focus(Some(parent)))
        });
    }

    let state_slot = wol_ladder_state();
    let mut state = state_slot.borrow_mut();
    state.parent_id = parent_id;
    state.button_back_id = button_back_id;
    state.window_ladder_id = window_ladder_id;
    state.parent = parent;
    state.button_back = button_back;
    state.window_ladder = window_ladder;
}

pub fn wol_ladder_screen_shutdown(layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    {
        let state_slot = wol_ladder_state();
        let state = state_slot.borrow();
        if let Some(window) = state.window_ladder.as_ref() {
            with_ladder_browser_mut(|browser| {
                browser.borrow_mut().close_browser_window(&window.borrow());
            });
        }
    }
    layout.hide(true);
    let _ = get_shell().shutdown_complete(None, false);
}

pub fn wol_ladder_screen_update(_layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {}

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

    let (parent, button_id) = {
        let slot = wol_ladder_state();
        let state = slot.borrow();
        (state.parent.clone(), state.button_back_id)
    };
    dispatch_esc_gadget_selected(parent, button_id);

    WindowMsgHandled::Handled
}

pub fn wol_ladder_screen_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::Create => WindowMsgHandled::Handled,
        WindowMessage::Destroy => WindowMsgHandled::Handled,
        WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
        WindowMessage::GadgetSelected => {
            let control_id = data1 as i32;
            let state_slot = wol_ladder_state();
            let state = state_slot.borrow_mut();
            if control_id == state.button_back_id {
                let _ = get_shell().pop();
                return WindowMsgHandled::Handled;
            }
            WindowMsgHandled::Handled
        }
        WindowMessage::GadgetEditDone => WindowMsgHandled::Handled,
        _ => WindowMsgHandled::Ignored,
    }
}
