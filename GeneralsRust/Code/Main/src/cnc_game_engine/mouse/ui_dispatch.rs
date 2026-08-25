#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

impl CnCGameEngine {
    /// Feed Main-owned OS keyboard state into GameClient Keyboard device residual.
    /// Main still owns command translation / hotkeys.
    /// Wave 606: via `host_inject_game_client_key`.
    #[cfg(feature = "game_client")]
    pub(in crate::cnc_game_engine) fn inject_game_client_key(
        &self,
        physical_key: &winit::keyboard::PhysicalKey,
        pressed: bool,
    ) {
        // Wave 606: thin wrapper — OS key inject via host helper.
        self.host_inject_game_client_key(physical_key, pressed);
    }

    /// Wave 606: host OS→GameClient key inject residual.
    #[cfg(feature = "game_client")]
    pub(in crate::cnc_game_engine) fn host_inject_game_client_key(
        &self,
        physical_key: &winit::keyboard::PhysicalKey,
        pressed: bool,
    ) {
        // Wave 606: host OS key inject residual.
        if let Some(code) = Self::to_game_client_key_code(physical_key) {
            game_client::input::keyboard::with_keyboard(|kb| {
                let _ = kb.handle_key_simple(code, pressed);
            });
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
            _ => return None,
        })
    }

    /// Feed Main-owned OS mouse state into GameClient Mouse device residual.
    /// Main still owns command translation; this keeps client device state honest
    /// for presentation-shell UI without dual OS event ownership.
    /// Wave 606: via `host_inject_game_client_mouse_move`.
    #[cfg(feature = "game_client")]
    pub(in crate::cnc_game_engine) fn inject_game_client_mouse_move(&self, x: f32, y: f32) {
        // Wave 606: thin wrapper — OS mouse move inject via host helper.
        self.host_inject_game_client_mouse_move(x, y);
    }

    /// Wave 606: host OS→GameClient mouse-move inject residual.
    #[cfg(feature = "game_client")]
    pub(in crate::cnc_game_engine) fn host_inject_game_client_mouse_move(&self, x: f32, y: f32) {
        // Wave 606: host OS mouse move inject residual.
        game_client::input::mouse::with_mouse(|mouse| {
            let _ = mouse.handle_mouse_move(x, y);
        });
    }

    /// Wave 606: via `host_inject_game_client_mouse_button`.
    #[cfg(feature = "game_client")]
    pub(in crate::cnc_game_engine) fn inject_game_client_mouse_button(
        &self,
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
        &self,
        physical_key: &winit::keyboard::PhysicalKey,
        pressed: bool,
    ) -> bool {
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::game_window::WindowInputReturnCode;
            let Some(vk) = Self::winit_physical_key_to_wnd_vk(physical_key) else {
                return false;
            };
            // C++ KEY_STATE_DOWN=0x02, KEY_STATE_UP=0x01
            let state = if pressed { 0x02 } else { 0x01 };
            game_client::gui::dispatch_os_key_to_window_manager(vk, state)
                == WindowInputReturnCode::Used
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = (physical_key, pressed);
            false
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
            _ => return None,
        })
    }

    pub(in crate::cnc_game_engine) fn dispatch_os_mouse_to_window_manager(
        &self,
        button: MouseButton,
        pressed: bool,
        x: i32,
        y: i32,
        origin: MouseInputOrigin,
    ) -> bool {
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::control_bar::{
                with_host_control_bar_input_provenance, HostControlBarInputProvenance,
            };
            use game_client::gui::game_window::{WindowInputReturnCode, WindowMessage};
            let msg = match (button, pressed) {
                (MouseButton::Left, true) => WindowMessage::LeftDown,
                (MouseButton::Left, false) => WindowMessage::LeftUp,
                (MouseButton::Right, true) => WindowMessage::RightDown,
                (MouseButton::Right, false) => WindowMessage::RightUp,
                (MouseButton::Middle, true) => WindowMessage::MiddleDown,
                (MouseButton::Middle, false) => WindowMessage::MiddleUp,
                _ => return false,
            };
            // `CommandSourceType::FromUser` alone cannot distinguish a real
            // winit event from `inject_winit_equivalent_*`: both deliberately
            // follow the same WND callback path. Scope the actual origin over
            // this synchronous dispatch so publication captures it exactly.
            let provenance = match origin {
                MouseInputOrigin::Physical => {
                    HostControlBarInputProvenance::PhysicalWindowMouseInput
                }
                MouseInputOrigin::Injected => HostControlBarInputProvenance::InjectedOrUnknown,
            };
            with_host_control_bar_input_provenance(provenance, || {
                game_client::gui::dispatch_os_mouse_to_window_manager(msg, x, y)
                    == WindowInputReturnCode::Used
            })
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = (button, pressed, x, y, origin);
            false
        }
    }

    pub(in crate::cnc_game_engine) fn dispatch_os_mouse_move(&self, x: i32, y: i32) -> bool {
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::game_window::{WindowInputReturnCode, WindowMessage};
            game_client::gui::dispatch_os_mouse_to_window_manager(WindowMessage::MousePos, x, y)
                == WindowInputReturnCode::Used
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = (x, y);
            false
        }
    }

    pub(in crate::cnc_game_engine) fn dispatch_os_mouse_wheel(
        &self,
        delta: &winit::event::MouseScrollDelta,
        x: i32,
        y: i32,
    ) -> bool {
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::game_window::{WindowInputReturnCode, WindowMessage};
            let lines = match delta {
                winit::event::MouseScrollDelta::LineDelta(_, y) => *y,
                winit::event::MouseScrollDelta::PixelDelta(p) => (p.y as f32) / 16.0,
            };
            if lines.abs() < f32::EPSILON {
                return false;
            }
            let msg = if lines > 0.0 {
                WindowMessage::WheelUp
            } else {
                WindowMessage::WheelDown
            };
            game_client::gui::dispatch_os_mouse_to_window_manager(msg, x, y)
                == WindowInputReturnCode::Used
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = (delta, x, y);
            false
        }
    }

    /// Wave 606: host OS→GameClient mouse-button inject residual.
    #[cfg(feature = "game_client")]
    pub(in crate::cnc_game_engine) fn host_inject_game_client_mouse_button(
        &self,
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
        game_client::input::mouse::with_mouse(|mouse| {
            let _ = mouse.handle_mouse_button(gc_btn, pressed, Instant::now());
        });
    }

    /// Wave 606: via `host_inject_game_client_mouse_scroll`.
    #[cfg(feature = "game_client")]
    pub(in crate::cnc_game_engine) fn inject_game_client_mouse_scroll(&self, delta_y: f32) {
        // Wave 606: thin wrapper — OS mouse scroll inject via host helper.
        self.host_inject_game_client_mouse_scroll(delta_y);
    }

    /// Wave 606: host OS→GameClient mouse-scroll inject residual.
    #[cfg(feature = "game_client")]
    pub(in crate::cnc_game_engine) fn host_inject_game_client_mouse_scroll(&self, delta_y: f32) {
        // Wave 606: host OS mouse scroll inject residual.
        game_client::input::mouse::with_mouse(|mouse| {
            let _ = mouse.handle_scroll_lines(delta_y);
        });
    }
}
