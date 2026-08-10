// Split from `message_stream/meta_event.rs` dump. Included by `meta_event_impl/mod.rs`.

#[derive(Default)]
pub struct MetaEventTranslator {
    last_key_down: u32,
    last_mod_state: u32,
    mouse_down_position: [ICoord2D; 3],
    next_up_creates_double: [bool; 3],
}

impl MetaEventTranslator {
    pub fn new() -> Self {
        ensure_meta_map_loaded();
        Self::default()
    }
}

impl GameMessageTranslator for MetaEventTranslator {
    fn translate_game_message(&mut self, msg: &GameMessage) -> GameMessageDisposition {
        let mut disp = GameMessageDisposition::KeepMessage;
        let msg_type = msg.get_type();

        if matches!(
            msg_type,
            GameMessageType::RawKeyDown(_) | GameMessageType::RawKeyUp(_)
        ) {
            let key = match msg.get_argument(0) {
                Some(GameMessageArgumentType::Integer(value)) => *value as u32,
                _ => match msg_type {
                    GameMessageType::RawKeyDown(code) | GameMessageType::RawKeyUp(code) => *code,
                    _ => 0,
                },
            };
            let key_state = match msg.get_argument(1) {
                Some(GameMessageArgumentType::Integer(value)) => *value as u32,
                _ => 0,
            };

            let mut new_mod_state = 0;
            if (key_state & KEY_STATE_CONTROL) != 0 {
                new_mod_state |= MOD_CTRL;
            }
            if (key_state & KEY_STATE_SHIFT) != 0 {
                new_mod_state |= MOD_SHIFT;
            }
            if (key_state & KEY_STATE_ALT) != 0 {
                new_mod_state |= MOD_ALT;
            }

            let shell_active = try_with_shell_mut(|shell| shell.is_shell_active()).unwrap_or(false);
            let client_frame =
                crate::core::game_client::with_live_game_client_mut(|client| client.get_frame())
                    .unwrap_or(0);

            let map_guard = get_meta_map().read().unwrap_or_else(|e| e.into_inner());
            for map in map_guard.iter() {
                // C++ parity: ignore game-only keybinds before the GameClient reaches frame 1.
                // This prevents load-screen input from getting stuck in frame-0 menu transitions.
                if map.usable_in == COMMANDUSABLE_GAME && client_frame < 1 {
                    continue;
                }
                if shell_active && (map.usable_in & COMMANDUSABLE_SHELL) == 0 {
                    continue;
                }
                if !shell_active && (map.usable_in & COMMANDUSABLE_GAME) == 0 {
                    continue;
                }

                if map.key == 0
                    && new_mod_state != self.last_mod_state
                    && ((map.transition == Transition::Up && map.mod_state == self.last_mod_state)
                        || (map.transition == Transition::Down && map.mod_state == new_mod_state))
                {
                    if let Some(new_disp) = dispatch_map_entry(map) {
                        disp = new_disp;
                        break;
                    }
                }

                let transition_matches = match map.transition {
                    Transition::Up => (key_state & KEY_STATE_UP) != 0,
                    Transition::Down => (key_state & KEY_STATE_DOWN) != 0,
                    // C++ currently disables DOUBLEDOWN generation in MetaEvent.cpp.
                    Transition::DoubleDown => false,
                };

                if map.key == key && map.mod_state == new_mod_state && transition_matches {
                    // C++ eats autorepeat for known keys but does not emit the meta-message.
                    if (key_state & KEY_STATE_AUTOREPEAT) != 0 {
                        disp = GameMessageDisposition::DestroyMessage;
                        break;
                    }

                    if let Some(new_disp) = dispatch_map_entry(map) {
                        disp = new_disp;
                        break;
                    }
                }
            }

            if matches!(msg_type, GameMessageType::RawKeyDown(_)) {
                self.last_key_down = key;
            }
            self.last_mod_state = new_mod_state;
        }

        match msg_type {
            GameMessageType::RawMousePosition(pos) => {
                apply_demo_camera_adjust_from_mouse_position(pos);
            }
            GameMessageType::RawMouseLeftButtonDown(pos, ..)
            | GameMessageType::RawMouseMiddleButtonDown(pos, ..)
            | GameMessageType::RawMouseRightButtonDown(pos, ..) => {
                let index = match msg_type {
                    GameMessageType::RawMouseMiddleButtonDown(..) => 1,
                    GameMessageType::RawMouseRightButtonDown(..) => 2,
                    _ => 0,
                };
                self.mouse_down_position[index] = pos.clone();
                self.next_up_creates_double[index] = false;
            }
            GameMessageType::RawMouseLeftDoubleClick(..)
            | GameMessageType::RawMouseMiddleDoubleClick(..)
            | GameMessageType::RawMouseRightDoubleClick(..) => {
                let index = match msg_type {
                    GameMessageType::RawMouseMiddleDoubleClick(..) => 1,
                    GameMessageType::RawMouseRightDoubleClick(..) => 2,
                    _ => 0,
                };
                self.next_up_creates_double[index] = true;
            }
            GameMessageType::RawMouseLeftButtonUp(pos, modifiers, ..)
            | GameMessageType::RawMouseMiddleButtonUp(pos, modifiers, ..)
            | GameMessageType::RawMouseRightButtonUp(pos, modifiers, ..) => {
                let index = match msg_type {
                    GameMessageType::RawMouseMiddleButtonUp(..) => 1,
                    GameMessageType::RawMouseRightButtonUp(..) => 2,
                    _ => 0,
                };

                let region = build_region(&self.mouse_down_position[index], pos);
                let mut region = IRegion2D {
                    x: region.x,
                    y: region.y,
                    width: region.width,
                    height: region.height,
                };

                if region.width.abs() < DRAG_TOLERANCE && region.height.abs() < DRAG_TOLERANCE {
                    region.width = 0;
                    region.height = 0;
                }

                let click_message = if self.next_up_creates_double[index] {
                    self.next_up_creates_double[index] = false;
                    match msg_type {
                        GameMessageType::RawMouseMiddleButtonUp(..) => {
                            GameMessageType::MouseMiddleDoubleClick(region, *modifiers)
                        }
                        GameMessageType::RawMouseRightButtonUp(..) => {
                            GameMessageType::MouseRightDoubleClick(region, *modifiers)
                        }
                        _ => GameMessageType::MouseLeftDoubleClick(region, *modifiers),
                    }
                } else {
                    match msg_type {
                        GameMessageType::RawMouseMiddleButtonUp(..) => {
                            GameMessageType::MouseMiddleClick(region, *modifiers)
                        }
                        GameMessageType::RawMouseRightButtonUp(..) => {
                            GameMessageType::MouseRightClick(region, *modifiers)
                        }
                        _ => GameMessageType::MouseLeftClick(region, *modifiers),
                    }
                };
                emit_message(GameMessage::new(click_message));
            }
            _ => {}
        }

        disp
    }
}
