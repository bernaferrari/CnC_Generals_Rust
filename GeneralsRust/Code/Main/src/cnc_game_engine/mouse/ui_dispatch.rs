#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

#[cfg(feature = "game_client")]
pub(super) fn window_message_for_mouse_wheel(
    delta: &winit::event::MouseScrollDelta,
) -> Option<game_client::gui::game_window::WindowMessage> {
    use game_client::gui::game_window::WindowMessage;

    match delta {
        winit::event::MouseScrollDelta::LineDelta(_, y) => {
            // Windows Winit LineDelta is the raw WM_MOUSEWHEEL integer / 120.
            // C++ emits no raw message only for raw zero; a nonzero raw delta
            // whose spin truncates to zero takes WindowXlat's else branch and
            // dispatches WHEEL_DOWN (Mouse.cpp:794-801; WindowXlat.cpp:106-112).
            if *y == 0.0 {
                None
            } else if y.trunc() > 0.0 {
                Some(WindowMessage::WheelUp)
            } else {
                Some(WindowMessage::WheelDown)
            }
        }
        // PixelDelta remains Main's platform extension; preserve its existing
        // /16 scaling and sign-only WND direction behavior.
        winit::event::MouseScrollDelta::PixelDelta(p) => {
            let lines = (p.y as f32) / 16.0;
            if lines.abs() < f32::EPSILON {
                None
            } else if lines > 0.0 {
                Some(WindowMessage::WheelUp)
            } else {
                Some(WindowMessage::WheelDown)
            }
        }
    }
}

impl CnCGameEngine {
    /// Feed Main-owned OS keyboard state into GameClient Keyboard device residual.
    /// Main still owns command translation / hotkeys.
    /// Wave 606: via `host_inject_game_client_key`.
    #[cfg(feature = "game_client")]
    pub(in crate::cnc_game_engine) fn inject_game_client_key(
        &mut self,
        physical_key: &winit::keyboard::PhysicalKey,
        pressed: bool,
    ) {
        // Wave 606: thin wrapper — OS key inject via host helper.
        self.host_inject_game_client_key(physical_key, pressed);
    }

    /// Wave 606: host OS→GameClient key inject residual.
    #[cfg(feature = "game_client")]
    pub(in crate::cnc_game_engine) fn host_inject_game_client_key(
        &mut self,
        physical_key: &winit::keyboard::PhysicalKey,
        pressed: bool,
    ) {
        // Wave 606: host OS key inject residual.
        if let Some(code) = Self::to_game_client_key_code(physical_key) {
            let _ = self.game_client.inject_keyboard_key(code, pressed);
        }
    }

    /// Map winit physical keys to GameClient KeyCode without sharing winit types across crates.
    #[cfg(feature = "game_client")]
    pub(in crate::cnc_game_engine) fn to_game_client_key_code(
        physical_key: &winit::keyboard::PhysicalKey,
    ) -> Option<game_client::input::KeyCode> {
        use game_client::input::KeyCode as Gk;
        use winit::keyboard::{KeyCode as Wk, PhysicalKey};
        let PhysicalKey::Code(code) = physical_key else {
            return None;
        };
        Some(match code {
            Wk::Escape => Gk::Escape,
            Wk::Enter | Wk::NumpadEnter => Gk::Enter,
            Wk::Space => Gk::Space,
            Wk::Tab => Gk::Tab,
            Wk::Backspace => Gk::Backspace,
            Wk::Delete => Gk::Delete,
            Wk::Home => Gk::Home,
            Wk::End => Gk::End,
            Wk::PageUp => Gk::PageUp,
            Wk::PageDown => Gk::PageDown,
            Wk::ArrowLeft => Gk::Left,
            Wk::ArrowRight => Gk::Right,
            Wk::ArrowUp => Gk::Up,
            Wk::ArrowDown => Gk::Down,
            Wk::ShiftLeft => Gk::LeftShift,
            Wk::ShiftRight => Gk::RightShift,
            Wk::ControlLeft => Gk::LeftCtrl,
            Wk::ControlRight => Gk::RightCtrl,
            Wk::AltLeft => Gk::LeftAlt,
            Wk::AltRight => Gk::RightAlt,
            Wk::KeyA => Gk::A,
            Wk::KeyB => Gk::B,
            Wk::KeyC => Gk::C,
            Wk::KeyD => Gk::D,
            Wk::KeyE => Gk::E,
            Wk::KeyF => Gk::F,
            Wk::KeyG => Gk::G,
            Wk::KeyH => Gk::H,
            Wk::KeyI => Gk::I,
            Wk::KeyJ => Gk::J,
            Wk::KeyK => Gk::K,
            Wk::KeyL => Gk::L,
            Wk::KeyM => Gk::M,
            Wk::KeyN => Gk::N,
            Wk::KeyO => Gk::O,
            Wk::KeyP => Gk::P,
            Wk::KeyQ => Gk::Q,
            Wk::KeyR => Gk::R,
            Wk::KeyS => Gk::S,
            Wk::KeyT => Gk::T,
            Wk::KeyU => Gk::U,
            Wk::KeyV => Gk::V,
            Wk::KeyW => Gk::W,
            Wk::KeyX => Gk::X,
            Wk::KeyY => Gk::Y,
            Wk::KeyZ => Gk::Z,
            Wk::Digit0 => Gk::Num0,
            Wk::Digit1 => Gk::Num1,
            Wk::Digit2 => Gk::Num2,
            Wk::Digit3 => Gk::Num3,
            Wk::Digit4 => Gk::Num4,
            Wk::Digit5 => Gk::Num5,
            Wk::Digit6 => Gk::Num6,
            Wk::Digit7 => Gk::Num7,
            Wk::Digit8 => Gk::Num8,
            Wk::Digit9 => Gk::Num9,
            Wk::F1 => Gk::F1,
            Wk::F2 => Gk::F2,
            Wk::F3 => Gk::F3,
            Wk::F4 => Gk::F4,
            Wk::F5 => Gk::F5,
            Wk::F6 => Gk::F6,
            Wk::F7 => Gk::F7,
            Wk::F8 => Gk::F8,
            Wk::F9 => Gk::F9,
            Wk::F10 => Gk::F10,
            Wk::F11 => Gk::F11,
            Wk::F12 => Gk::F12,
            Wk::Numpad0 => Gk::NumPad0,
            Wk::Numpad1 => Gk::NumPad1,
            Wk::Numpad2 => Gk::NumPad2,
            Wk::Numpad3 => Gk::NumPad3,
            Wk::Numpad4 => Gk::NumPad4,
            Wk::Numpad5 => Gk::NumPad5,
            Wk::Numpad6 => Gk::NumPad6,
            Wk::Numpad7 => Gk::NumPad7,
            Wk::Numpad8 => Gk::NumPad8,
            Wk::Numpad9 => Gk::NumPad9,
            Wk::NumpadAdd => Gk::NumPadAdd,
            Wk::NumpadSubtract => Gk::NumPadSubtract,
            Wk::NumpadMultiply => Gk::NumPadMultiply,
            Wk::NumpadDivide => Gk::NumPadDivide,
            Wk::NumpadDecimal => Gk::NumPadDecimal,
            Wk::Minus => Gk::Minus,
            Wk::Equal => Gk::Plus,
            Wk::BracketLeft => Gk::LeftBracket,
            Wk::BracketRight => Gk::RightBracket,
            Wk::Semicolon => Gk::Semicolon,
            Wk::Quote => Gk::Quote,
            Wk::Backquote => Gk::Grave,
            Wk::Backslash => Gk::Backslash,
            Wk::Slash => Gk::Slash,
            Wk::Comma => Gk::Comma,
            Wk::Period => Gk::Period,
            Wk::Insert => Gk::Insert,
            _ => return None,
        })
    }

    /// Feed Main-owned OS mouse state into GameClient Mouse device residual.
    /// Main still owns command translation; this keeps client device state honest
    /// for presentation-shell UI without dual OS event ownership.
    /// Wave 606: via `host_inject_game_client_mouse_move`.
    #[cfg(feature = "game_client")]
    pub(in crate::cnc_game_engine) fn inject_game_client_mouse_move(&mut self, x: f32, y: f32) {
        // Wave 606: thin wrapper — OS mouse move inject via host helper.
        self.host_inject_game_client_mouse_move(x, y);
    }

    /// Wave 606: host OS→GameClient mouse-move inject residual.
    #[cfg(feature = "game_client")]
    pub(in crate::cnc_game_engine) fn host_inject_game_client_mouse_move(
        &mut self,
        x: f32,
        y: f32,
    ) {
        // Wave 606: host OS mouse move inject residual.
        if let Some(mouse) = self.game_client.input_mouse_mut() {
            let _ = mouse.handle_mouse_move(x, y);
        }
    }

    /// Feed wheel state into this client's input device.
    #[cfg(feature = "game_client")]
    pub(in crate::cnc_game_engine) fn inject_game_client_mouse_scroll(&mut self, delta_y: f32) {
        if let Some(mouse) = self.game_client.input_mouse_mut() {
            let _ = mouse.handle_scroll_lines(delta_y);
        }
    }

    /// Wave 606: via `host_inject_game_client_mouse_button`.
    #[cfg(feature = "game_client")]
    pub(in crate::cnc_game_engine) fn inject_game_client_mouse_button(
        &mut self,
        button: MouseButton,
        pressed: bool,
    ) {
        // Wave 606: thin wrapper — OS mouse button inject via host helper.
        self.host_inject_game_client_mouse_button(button, pressed);
    }

    /// C++ WindowXlat: OS button → `TheWindowManager` gadget hit-test.
    /// Returns true when the WND consumed the click (shell active or gadget Used).
    /// C++ WindowXlat RAW_KEY → focused gadget `GWM_CHAR`.
    pub(in crate::cnc_game_engine) fn dispatch_os_key_to_window_manager(
        &mut self,
        physical_key: &winit::keyboard::PhysicalKey,
        pressed: bool,
    ) -> Result<bool, String> {
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::game_window::WindowInputReturnCode;
            let Some(vk) = Self::winit_physical_key_to_wnd_vk(physical_key) else {
                return Ok(false);
            };
            // C++ KEY_STATE_DOWN=0x02, KEY_STATE_UP=0x01
            let state = if pressed { 0x02 } else { 0x01 };
            let replay = matches!(
                self.game_logic.game_mode(),
                crate::game_logic::GameMode::Replay
            );
            let game_ending = self.game_logic.owned_script_game_ending();
            let tooltips_disabled = self.receiving_tooltips_disabled();
            let observer_roster = self.receiving_observer_roster();
            let mut replay_operation =
                game_client::gui::callbacks::replay_menu::ingress::ReplayMenuOperation::new(
                    !self.game_logic.isInNetworkGame(),
                    self.game_client.window_manager_owner().downgrade(),
                );
            let camera_rules = self.game_logic.camera_input_rules();
            let upgrade_tooltip = self.game_logic.receiving_upgrade_tooltip_observer();
            let menu_world = self.receiving_menu_world_facts();
            let control_bar_observation = self.receiving_control_bar_callback_observation();
            self.menu_audio
                .require_bound()
                .expect("native input admission requires initialized menu audio");
            let menu_audio_locality = self.game_logic.receiving_audio_locality_for_player(-1);
            let menu_audio_view =
                super::super::audio_view::CompletedAudioView::capture(&self.game_logic);
            let mut menu_audio = self.menu_audio.lend(
                &mut self.game_logic.session_view.audio_random,
                self.game_logic.queued_audio_events.policy(),
                menu_audio_locality,
                menu_audio_view,
            );
            let pending = &mut self.game_logic.replay_pending;
            let mut replay_version = |filename: &str| {
                pending
                    .replay_version_differs_owned(filename.to_owned())
                    .unwrap_or(false)
            };
            let render_pipeline = &mut self.render_pipeline;
            let mut terrain_lod = |reduction| {
                if let Some(terrain) = render_pipeline.native_terrain_mut() {
                    terrain.apply_texture_lod_reduction(reduction);
                }
            };
            let mut emitted_messages = Vec::new();
            let (
                window_manager_owner,
                campaign_manager,
                main_menu,
                menu_host,
                display,
                videos,
                ime,
                mut menus,
                ui,
                cursor_mouse,
            ) = self.game_client.window_owner_campaign_menu_and_host_mut();
            let mut context = game_client::gui::window_manager::ControlBarInputContext {
            control_bar_observation:Some(control_bar_observation),
            diplomacy_world: None,
            menu_audio: Some(&mut menu_audio),
            input_provenance: game_client::gui::control_bar::HostControlBarInputProvenance::InjectedOrUnknown,
            ui, logic_frame: Some(menu_world.logic_frame), client_frame: Some(menu_world.client_frame),
            menus: Some(menus.reborrow()),
            ime:Some(ime),
            upgrade_tooltip: game_client::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource::Receiving(&upgrade_tooltip),
                replay_menu: Some(&mut replay_operation),
                replay_version: Some(&mut replay_version),
                observer_roster: observer_roster.as_ref(),
                camera_options: Some(
                    game_client::gui::window_manager::CameraOptionsInputContext::new(
                        self.ui_manager.options_preferences_mut(),
                        camera_rules,
                    )
                    .with_terrain_lod(&mut terrain_lod),
                ),
                control_bar: &mut self.control_bar,
                viewport: &mut self.game_logic.session_view.camera,
                replay,
                game_ending,
                tooltips_disabled,
                cursor_mouse,
                emitted_messages: Some(&mut emitted_messages),
                campaign_manager: Some(campaign_manager),
                main_menu: Some(main_menu),
                window_manager_owner: Some(window_manager_owner),
                menu_world: Some(menu_world),
                menu_host: Some(menu_host),
                script_display: Some(display),
                window_videos: Some(&mut videos.challenge),
            in_game_movies: Some(&mut videos.in_game),
            };
            let result = game_client::gui::window_manager::dispatch_os_key_to_window_manager_with_control_bar_for_owner(
                window_manager_owner,
                vk,
                state,
                &mut context,
            );
            drop(context);
            drop(replay_version);
            self.complete_receiving_menu_event_messages(emitted_messages)?;
            if let Some(step) = replay_operation.take() {
                step.complete(self)?;
            }
            Ok(result == WindowInputReturnCode::Used)
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = (physical_key, pressed);
            Ok(false)
        }
    }

    pub(in crate::cnc_game_engine) fn winit_physical_key_to_wnd_vk(
        physical_key: &winit::keyboard::PhysicalKey,
    ) -> Option<u8> {
        use winit::keyboard::{KeyCode as Wk, PhysicalKey};
        let PhysicalKey::Code(code) = physical_key else {
            return None;
        };
        Some(match code {
            Wk::Escape => 0x1B,
            Wk::Enter | Wk::NumpadEnter => 13,
            Wk::Space => 32,
            Wk::Tab => 9,
            Wk::Backspace => 8,
            Wk::Delete => 0x2E,
            Wk::Home => 36,
            Wk::End => 35,
            Wk::PageUp => 33,
            Wk::PageDown => 34,
            Wk::ArrowLeft => 37,
            Wk::ArrowUp => 38,
            Wk::ArrowRight => 39,
            Wk::ArrowDown => 40,
            Wk::KeyA => b'A',
            Wk::KeyB => b'B',
            Wk::KeyC => b'C',
            Wk::KeyD => b'D',
            Wk::KeyE => b'E',
            Wk::KeyF => b'F',
            Wk::KeyG => b'G',
            Wk::KeyH => b'H',
            Wk::KeyI => b'I',
            Wk::KeyJ => b'J',
            Wk::KeyK => b'K',
            Wk::KeyL => b'L',
            Wk::KeyM => b'M',
            Wk::KeyN => b'N',
            Wk::KeyO => b'O',
            Wk::KeyP => b'P',
            Wk::KeyQ => b'Q',
            Wk::KeyR => b'R',
            Wk::KeyS => b'S',
            Wk::KeyT => b'T',
            Wk::KeyU => b'U',
            Wk::KeyV => b'V',
            Wk::KeyW => b'W',
            Wk::KeyX => b'X',
            Wk::KeyY => b'Y',
            Wk::KeyZ => b'Z',
            Wk::Digit0 => b'0',
            Wk::Digit1 => b'1',
            Wk::Digit2 => b'2',
            Wk::Digit3 => b'3',
            Wk::Digit4 => b'4',
            Wk::Digit5 => b'5',
            Wk::Digit6 => b'6',
            Wk::Digit7 => b'7',
            Wk::Digit8 => b'8',
            Wk::Digit9 => b'9',
            Wk::F1 => 0x70,
            Wk::F2 => 0x71,
            Wk::F3 => 0x72,
            Wk::F4 => 0x73,
            Wk::F5 => 0x74,
            Wk::F6 => 0x75,
            Wk::F7 => 0x76,
            Wk::F8 => 0x77,
            Wk::F9 => 0x78,
            Wk::F10 => 0x79,
            Wk::F11 => 0x7A,
            Wk::F12 => 0x7B,
            Wk::ShiftLeft => 0xA0,
            Wk::ShiftRight => 0xA1,
            Wk::ControlLeft => 0xA2,
            Wk::ControlRight => 0xA3,
            Wk::AltLeft => 0xA4,
            Wk::AltRight => 0xA5,
            Wk::Numpad0 => 0x60,
            Wk::Numpad1 => 0x61,
            Wk::Numpad2 => 0x62,
            Wk::Numpad3 => 0x63,
            Wk::Numpad4 => 0x64,
            Wk::Numpad5 => 0x65,
            Wk::Numpad6 => 0x66,
            Wk::Numpad7 => 0x67,
            Wk::Numpad8 => 0x68,
            Wk::Numpad9 => 0x69,
            Wk::NumpadMultiply => 0x6A,
            Wk::NumpadAdd => 0x6B,
            Wk::NumpadSubtract => 0x6D,
            Wk::NumpadDecimal => 0x6E,
            Wk::NumpadDivide => 0x6F,
            Wk::Minus => 0xBD,
            Wk::Equal => 0xBB,
            Wk::BracketLeft => 0xDB,
            Wk::BracketRight => 0xDD,
            Wk::Semicolon => 0xBA,
            Wk::Quote => 0xDE,
            Wk::Backquote => 0xC0,
            Wk::Backslash => 0xDC,
            Wk::Comma => 0xBC,
            Wk::Period => 0xBE,
            Wk::Slash => 0xBF,
            Wk::Insert => 0x2D,
            _ => return None,
        })
    }

    #[cfg(feature = "game_client")]
    fn native_window_mouse_facts(&self) -> game_client::gui::WindowMouseFacts {
        game_client::gui::WindowMouseFacts {
            mouse_locked: self.look_at_modes.look_at_host_mouse_locked(),
            scrolling: self.look_at_modes.look_at_host_is_scrolling(),
        }
    }

    #[cfg(feature = "game_client")]
    pub(in crate::cnc_game_engine) fn receiving_tooltips_disabled(&self) -> bool {
        self.game_client
            .in_game_ui_subsystem()
            .map(|ui| ui.tooltips_disabled_at(self.game_logic.frame))
            .unwrap_or(true)
    }

    #[cfg(feature = "game_client")]
    fn dispatch_native_window_mouse_with_bar(
        &mut self,
        msg: game_client::gui::game_window::WindowMessage,
        x: i32,
        y: i32,
        facts: game_client::gui::WindowMouseFacts,
        input_provenance: game_client::gui::control_bar::HostControlBarInputProvenance,
    ) -> Result<game_client::gui::game_window::WindowInputReturnCode, String> {
        let observation = self.receiving_control_bar_callback_observation();
        if observation.mouse.is_none() && !(facts.mouse_locked && game_client::message_stream::window_xlat::should_skip_os_window_manager_while_mouse_locked(msg,facts.scrolling)) {
            let requires_mouse = self
                .game_client
                .window_manager_owner_ref()
                .with_manager_ref(|manager| {
                    let mut hit = manager.get_window_under_cursor(x, y, false);
                    while let Some(window) = hit {
                        let window = window.borrow();
                        if window.get_name() == "ControlBar.wnd:LeftHUD" {
                            return true;
                        }
                        hit = window.get_parent();
                    }
                    false
                });
            if requires_mouse {
                return Err("native LeftHUD requires its receiving mouse service".into());
            }
        }
        let replay = matches!(
            self.game_logic.game_mode(),
            crate::game_logic::GameMode::Replay
        );
        let game_ending = self.game_logic.owned_script_game_ending();
        let tooltips_disabled = self.receiving_tooltips_disabled();
        let observer_roster = self.receiving_observer_roster();
        let mut replay_operation =
            game_client::gui::callbacks::replay_menu::ingress::ReplayMenuOperation::new(
                !self.game_logic.isInNetworkGame(),
                self.game_client.window_manager_owner().downgrade(),
            );
        let camera_rules = self.game_logic.camera_input_rules();
        let upgrade_tooltip = self.game_logic.receiving_upgrade_tooltip_observer();
        let menu_world = self.receiving_menu_world_facts();
        let control_bar_observation = self.receiving_control_bar_callback_observation();
        self.menu_audio
            .require_bound()
            .expect("native input admission requires initialized menu audio");
        let menu_audio_locality = self.game_logic.receiving_audio_locality_for_player(-1);
        let menu_audio_view =
            super::super::audio_view::CompletedAudioView::capture(&self.game_logic);
        let mut menu_audio = self.menu_audio.lend(
            &mut self.game_logic.session_view.audio_random,
            self.game_logic.queued_audio_events.policy(),
            menu_audio_locality,
            menu_audio_view,
        );
        let pending = &mut self.game_logic.replay_pending;
        let mut replay_version = |filename: &str| {
            pending
                .replay_version_differs_owned(filename.to_owned())
                .unwrap_or(false)
        };
        let render_pipeline = &mut self.render_pipeline;
        let mut terrain_lod = |reduction| {
            if let Some(terrain) = render_pipeline.native_terrain_mut() {
                terrain.apply_texture_lod_reduction(reduction);
            }
        };
        let mut emitted_messages = Vec::new();
        let (
            receiving_window_manager,
            campaign_manager,
            main_menu,
            menu_host,
            display,
            videos,
            ime,
            mut menus,
            ui,
            cursor_mouse,
        ) = self.game_client.window_owner_campaign_menu_and_host_mut();
        let mut context = game_client::gui::window_manager::ControlBarInputContext {
            control_bar_observation:Some(control_bar_observation),
            diplomacy_world: None,
            menu_audio: Some(&mut menu_audio),
            input_provenance,
            ui, logic_frame: Some(menu_world.logic_frame), client_frame: Some(menu_world.client_frame),
            menus: Some(menus.reborrow()),
            ime:Some(ime),
            upgrade_tooltip: game_client::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource::Receiving(&upgrade_tooltip),
            replay_menu: Some(&mut replay_operation),
            replay_version: Some(&mut replay_version),
            observer_roster: observer_roster.as_ref(),
            camera_options: Some(
                game_client::gui::window_manager::CameraOptionsInputContext::new(
                    self.ui_manager.options_preferences_mut(),
                    camera_rules,
                )
                .with_terrain_lod(&mut terrain_lod),
            ),
            control_bar: &mut self.control_bar,
            viewport: &mut self.game_logic.session_view.camera,
            replay,
            game_ending,
            tooltips_disabled,
            cursor_mouse,
            emitted_messages: Some(&mut emitted_messages),
            campaign_manager: Some(campaign_manager),
            main_menu: Some(main_menu),
            window_manager_owner: Some(receiving_window_manager),
            menu_world: Some(menu_world),
            menu_host: Some(menu_host),
            script_display: Some(display),
            window_videos: Some(&mut videos.challenge),
            in_game_movies: Some(&mut videos.in_game),
        };
        let result = game_client::gui::window_manager::dispatch_os_mouse_to_window_manager_with_control_bar_for_owner(
            receiving_window_manager,
            msg,
            x,
            y,
            facts,
            &mut context,
        );
        drop(context);
        drop(replay_version);
        self.complete_receiving_menu_event_messages(emitted_messages)?;
        if let Some(step) = replay_operation.take() {
            step.complete(self)?;
        }
        Ok(result)
    }

    pub(in crate::cnc_game_engine) fn dispatch_os_mouse_to_window_manager(
        &mut self,
        button: MouseButton,
        pressed: bool,
        x: i32,
        y: i32,
        origin: MouseInputOrigin,
    ) -> Result<bool, String> {
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::control_bar::HostControlBarInputProvenance;
            use game_client::gui::game_window::{WindowInputReturnCode, WindowMessage};
            let msg = match (button, pressed) {
                (MouseButton::Left, true) => WindowMessage::LeftDown,
                (MouseButton::Left, false) => WindowMessage::LeftUp,
                (MouseButton::Right, true) => WindowMessage::RightDown,
                (MouseButton::Right, false) => WindowMessage::RightUp,
                (MouseButton::Middle, true) => WindowMessage::MiddleDown,
                (MouseButton::Middle, false) => WindowMessage::MiddleUp,
                _ => return Ok(false),
            };
            // `CommandSourceType::FromUser` alone cannot distinguish a real
            // winit event from `inject_winit_equivalent_*`: both deliberately
            // follow the same WND callback path. Pass the actual origin into
            // this receiving synchronous dispatch so publication retains it exactly.
            let provenance = match origin {
                MouseInputOrigin::Physical => {
                    HostControlBarInputProvenance::PhysicalWindowMouseInput
                }
                MouseInputOrigin::Injected => HostControlBarInputProvenance::InjectedOrUnknown,
            };
            let facts = self.native_window_mouse_facts();
            self.dispatch_native_window_mouse_with_bar(msg, x, y, facts, provenance)
                .map(|result| result == WindowInputReturnCode::Used)
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = (button, pressed, x, y, origin);
            Ok(false)
        }
    }

    pub(in crate::cnc_game_engine) fn dispatch_os_mouse_move(
        &mut self,
        x: i32,
        y: i32,
    ) -> Result<bool, String> {
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::game_window::{WindowInputReturnCode, WindowMessage};
            Ok(self.dispatch_native_window_mouse_with_bar(
                WindowMessage::MousePos,
                x,
                y,
                self.native_window_mouse_facts(),
                game_client::gui::control_bar::HostControlBarInputProvenance::InjectedOrUnknown,
            )? == WindowInputReturnCode::Used)
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = (x, y);
            Ok(false)
        }
    }

    pub(in crate::cnc_game_engine) fn dispatch_os_mouse_wheel(
        &mut self,
        delta: &winit::event::MouseScrollDelta,
        x: i32,
        y: i32,
    ) -> Result<bool, String> {
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::game_window::WindowInputReturnCode;
            let Some(msg) = window_message_for_mouse_wheel(delta) else {
                return Ok(false);
            };
            Ok(self.dispatch_native_window_mouse_with_bar(
                msg,
                x,
                y,
                self.native_window_mouse_facts(),
                game_client::gui::control_bar::HostControlBarInputProvenance::InjectedOrUnknown,
            )? == WindowInputReturnCode::Used)
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = (delta, x, y);
            Ok(false)
        }
    }

    /// Wave 606: host OS→GameClient mouse-button inject residual.
    #[cfg(feature = "game_client")]
    pub(in crate::cnc_game_engine) fn host_inject_game_client_mouse_button(
        &mut self,
        button: MouseButton,
        pressed: bool,
    ) {
        // Wave 606: host OS mouse button inject residual.
        use game_client::input::mouse::MouseButton as GcMouseButton;
        use std::time::Instant;
        let gc_btn = match button {
            MouseButton::Left => GcMouseButton::Left,
            MouseButton::Right => GcMouseButton::Right,
            MouseButton::Middle => GcMouseButton::Middle,
            MouseButton::Back => GcMouseButton::Other(3),
            MouseButton::Forward => GcMouseButton::Other(4),
            MouseButton::Other(n) => GcMouseButton::Other(n as u16),
        };
        if let Some(mouse) = self.game_client.input_mouse_mut() {
            let _ = mouse.handle_mouse_button(gc_btn, pressed, Instant::now());
        }
    }
}
