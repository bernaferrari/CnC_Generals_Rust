//! Credits playback owned by the actual receiving layout's parent window.
//! No process menu slot, synthetic window ID, or global renderer is selected.
use crate::credits::CreditsManager;
use crate::gui::game_window::WindowEventEffects;
use crate::gui::window_draw_context::WindowDrawContext;
use crate::gui::window_manager::{WindowManager, WindowManagerOwner};
use crate::gui::{
    GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled, WindowStatus,
};
use std::{cell::RefCell, rc::Rc};

const PARENT: &str = "CreditsMenu.wnd:ParentCreditsWindow";
struct CreditsWindowState {
    credits: CreditsManager,
}
fn find_in(window: &Rc<RefCell<GameWindow>>) -> Option<Rc<RefCell<GameWindow>>> {
    let borrowed = window.borrow();
    if borrowed.get_name().eq_ignore_ascii_case(PARENT) {
        return Some(window.clone());
    }
    for child in borrowed.children() {
        if let Some(found) = find_in(child) {
            return Some(found);
        }
    }
    None
}
fn parent(layout: &WindowLayout) -> Option<Rc<RefCell<GameWindow>>> {
    layout.windows().iter().find_map(find_in)
}
fn queue_pop(effects: &mut WindowEventEffects<'_>) {
    effects
        .window_manager_owner
        .expect("credits requires receiving shell owner")
        .queue_shell_operation_with_effects(|shell, effects| {
            if let Err(error) = shell.pop(effects) {
                log::warn!("credits pop failed: {error}");
            }
        });
}
pub(crate) fn init(layout: &WindowLayout, effects: &mut WindowEventEffects<'_>) {
    effects.request_shell_map(false);
    let Some(parent) = parent(layout) else {
        log::warn!("receiving Credits layout has no parent window");
        return;
    };
    let mut credits = CreditsManager::new();
    if let Err(error) = credits.load_from_path("Data/INI/Credits.ini") {
        log::warn!("credits load failed: {error}");
    }
    credits.init();
    parent
        .borrow_mut()
        .set_user_data(CreditsWindowState { credits });
    layout.hide_receiving(false, effects);
    // CreditsMenu.cpp:78 sets focus synchronously after the parent data loan.
    effects.with_manager(|manager, effects| {
        let _ = manager.set_focus(Some(&parent), effects);
    });
    if let Some(audio) = gamelogic::helpers::TheAudio::get() {
        audio.remove_audio_event(0xFFFF_FFF1);
        let mut event = gamelogic::common::audio::AudioEventRts::new("Credits");
        event.set_should_fade(true);
        let _ = audio.add_audio_event(&event);
    }
}
pub(crate) fn update(layout: &WindowLayout, effects: &mut WindowEventEffects<'_>) {
    let Some(parent) = parent(layout) else {
        queue_pop(effects);
        return;
    };
    effects.queue_window_operation(move |manager, effects| {
        // Focus callbacks run before the playback borrow, just as C++ does.
        let _ = manager.set_focus(Some(&parent), effects);
        let size = manager.screen_size();
        let finished = {
            let mut window = parent.borrow_mut();
            if window.get_status().contains(WindowStatus::DESTROYED) {
                return;
            }
            match window.get_user_data_mut::<CreditsWindowState>() {
                Some(state) => {
                    state.credits.update(size);
                    state.credits.is_finished()
                }
                None => true,
            }
        };
        // No window/playback borrow crosses shell callbacks or teardown.
        if finished {
            queue_pop(effects);
        }
    });
}
pub(crate) fn shutdown(layout: &WindowLayout, effects: &mut WindowEventEffects<'_>) {
    if let Some(parent) = parent(layout) {
        let mut window = parent.borrow_mut();
        if let Some(state) = window.get_user_data_mut::<CreditsWindowState>() {
            state.credits.reset();
        }
        window.set_user_data(());
    }
    effects.request_shell_map(true);
    layout.hide_receiving(true, effects);
    effects
        .window_manager_owner
        .expect("credits shutdown requires receiving shell")
        .queue_shell_operation_with_effects(|shell, effects| {
            if let Err(error) = shell.shutdown_complete(None, false, effects) {
                log::warn!("credits shutdown failed: {error}");
            }
        });
    if let Some(audio) = gamelogic::helpers::TheAudio::get() {
        audio.remove_audio_event(0xFFFF_FFF1);
    }
}
pub(crate) fn draw(window: &GameWindow, context: &mut WindowDrawContext<'_>) {
    if let Some(state) = window.get_user_data::<CreditsWindowState>() {
        state.credits.draw(context);
    }
}
pub(crate) fn system(
    _window: &GameWindow,
    msg: WindowMessage,
    d1: WindowMsgData,
    d2: WindowMsgData,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::Create | WindowMessage::Destroy | WindowMessage::GadgetSelected => {
            WindowMsgHandled::Handled
        }
        WindowMessage::InputFocus => crate::gui::write_input_focus_response(d1, d2, true),
        _ => WindowMsgHandled::Ignored,
    }
}
pub(crate) fn input(
    _window: &GameWindow,
    msg: WindowMessage,
    d1: WindowMsgData,
    d2: WindowMsgData,
    effects: &mut WindowEventEffects<'_>,
) -> WindowMsgHandled {
    if msg == WindowMessage::Char && d1 == 0x1B {
        if d2 & 1 != 0 {
            queue_pop(effects);
        }
        WindowMsgHandled::Handled
    } else {
        WindowMsgHandled::Ignored
    }
}
/// Tooling operations borrow the same actual playback, never a private mirror.
pub fn with_credits_for_owner<R>(
    owner: &WindowManagerOwner,
    operation: impl FnOnce(&mut CreditsManager, (i32, i32)) -> R,
) -> Option<R> {
    owner
        .try_with_manager(|manager| {
            let window = manager.find_window_by_name(PARENT)?;
            let size = manager.screen_size();
            let mut window = window.borrow_mut();
            let state = window.get_user_data_mut::<CreditsWindowState>()?;
            Some(operation(&mut state.credits, size))
        })
        .flatten()
}
