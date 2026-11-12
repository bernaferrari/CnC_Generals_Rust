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
    start_credits_music(effects);
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
    stop_credits_music(effects);
}
/// C++ CreditsMenuInit: `removeAudioEvent(AHSV_StopTheMusicFade)`, then the
/// fading "Credits" track, in this client's menu audio service.
pub(crate) fn start_credits_music(effects: &mut WindowEventEffects<'_>) {
    if effects.has_menu_audio() {
        effects.stop_the_music_fade();
        effects.play_fading_menu_audio("Credits");
    }
}
/// C++ CreditsMenuShutdown: `removeAudioEvent(AHSV_StopTheMusicFade)`.
pub(crate) fn stop_credits_music(effects: &mut WindowEventEffects<'_>) {
    if effects.has_menu_audio() {
        effects.stop_the_music_fade();
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

/// Test/tooling: attach an empty credits playback owner to the named parent.
pub fn attach_credits_playback_for_owner(owner: &WindowManagerOwner) -> bool {
    owner
        .try_with_manager(|manager| {
            let window = manager.find_window_by_name(PARENT)?;
            window
                .borrow_mut()
                .set_user_data(CreditsWindowState {
                    credits: CreditsManager::new(),
                });
            Some(())
        })
        .flatten()
        .is_some()
}

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

#[cfg(test)]
mod credits_music_tests {
    use super::*;
    use crate::gui::menu_audio::{MenuAudioEffects, MenuAudioHandle, MenuAudioStop};

    #[derive(Default)]
    struct Recorder(Vec<String>);
    impl MenuAudioEffects for Recorder {
        fn is_bound(&self) -> bool {
            true
        }
        fn play(&mut self, name: &str) -> Option<MenuAudioHandle> {
            self.0.push(format!("play {name}"));
            None
        }
        fn play_fading(&mut self, name: &str) -> Option<MenuAudioHandle> {
            self.0.push(format!("fade-in {name}"));
            None
        }
        fn stop(&mut self, _: MenuAudioHandle) -> MenuAudioStop {
            MenuAudioStop::NotOwned
        }
        fn pump(&mut self) {}
        fn fade_out_music(&mut self) {
            self.0.push("fade-menu-music".into());
        }
        fn stop_the_music_fade(&mut self) {
            self.0.push("stop-music-fade".into());
        }
        fn set_system_volume(&mut self, _: f32, _: game_engine::common::audio::AudioAffect) {}
    }

    #[test]
    fn credits_music_fades_out_the_menu_track_then_fades_in_credits_in_its_client() {
        crate::test_support::ensure_ambient_host();
        let mut messages = Vec::new();
        let mut effects = WindowEventEffects::new(&mut messages);
        let mut audio = Recorder::default();
        effects.with_menu_audio(&mut audio, |effects| {
            start_credits_music(effects);
            stop_credits_music(effects);
        });
        assert_eq!(
            audio.0,
            ["stop-music-fade", "fade-in Credits", "stop-music-fade"],
            "CreditsMenu.cpp init/shutdown order"
        );
    }

    #[test]
    fn credits_music_without_a_client_audio_service_plays_nowhere() {
        crate::test_support::ensure_ambient_host();
        let mut messages = Vec::new();
        let mut effects = WindowEventEffects::new(&mut messages);
        start_credits_music(&mut effects);
        stop_credits_music(&mut effects);
    }
}
