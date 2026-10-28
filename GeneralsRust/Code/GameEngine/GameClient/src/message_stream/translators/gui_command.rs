use super::*;

/// GUI Command Translator - handles UI-specific commands
pub struct GUICommandTranslator {
    pub(super) ui_state: HashMap<String, bool>,
}

impl GUICommandTranslator {
    pub fn new() -> Self {
        Self {
            ui_state: HashMap::new(),
        }
    }

    pub(super) fn toggle_flag(&mut self, key: &str, default: bool) -> bool {
        let current = *self.ui_state.get(key).unwrap_or(&default);
        self.ui_state.insert(key.to_string(), !current);
        !current
    }

    pub(super) fn handle_toggle_control_bar(
        &mut self,
        dispatch: &mut game_engine::common::message_stream::WindowInputDispatchFn<'_>,

        ui: &mut crate::message_stream::ReceivingUi<'_>,
    ) -> Vec<GameMessageType> {
        let new_state = self.toggle_flag("control_bar_visible", true);
        info!("Toggling control bar to: {}", new_state);
        if !crate::message_stream::meta_event::dispatch_ui_input(
            game_engine::common::message_stream::UiInputAction::ToggleControlBar,
            dispatch,
            ui,
        ) {
            warn!("Receiving control-bar operation did not complete");
        }
        vec![] // UI changes don't generate game messages
    }

    pub(super) fn handle_toggle_diplomacy(
        &mut self,
        dispatch: &mut game_engine::common::message_stream::WindowInputDispatchFn<'_>,

        ui: &mut crate::message_stream::ReceivingUi<'_>,
    ) -> Vec<GameMessageType> {
        let new_state = self.toggle_flag("diplomacy_visible", false);
        info!("Toggling diplomacy to: {}", new_state);
        let result = ui.dispatch(
            dispatch,
            game_engine::common::message_stream::WindowInputDispatch::Ui(
                game_engine::common::message_stream::UiInputAction::ToggleDiplomacy {
                    immediate: false,
                },
            ),
        );
        if result != Some(game_engine::common::message_stream::WindowInputDispatchResult::Ui(true))
        {
            warn!("Receiving diplomacy operation did not complete");
        }
        vec![]
    }

    pub(super) fn clear_pending_gui_command_mode(
        &self,
        ui: &mut crate::message_stream::ReceivingUi<'_>,
    ) {
        ui.clear_pending_command();
        ui.clear_pending_special_power();
        ui.clear_attack_move_to_mode();
    }

    pub(super) fn handle_pending_non_context_gui_click(
        &mut self,
        region: &IRegion2D,
        window_query: &mut dyn FnMut(i32, i32, &mut crate::message_stream::ReceivingUi<'_>) -> bool,
        ui: &mut crate::message_stream::ReceivingUi<'_>,
    ) -> GameMessageDisposition {
        let Some(pending) = ui.get_pending_command() else {
            return GameMessageDisposition::KeepMessage;
        };
        if !is_pending_gui_non_context_command(&pending) {
            return GameMessageDisposition::KeepMessage;
        }

        // C++ GUICommandTranslator uses pixelRegion.hi as click location.
        let click_pos = ICoord2D::new(region.x + region.width, region.y + region.height);
        let click_region = IRegion2D {
            x: click_pos.x,
            y: click_pos.y,
            width: 0,
            height: 0,
        };

        let world = screen_to_terrain(&click_pos).unwrap_or(Coord3D {
            x: click_pos.x as f32,
            y: click_pos.y as f32,
            z: 0.0,
        });

        let local_player = get_local_player_id();
        let local_player_u32 = if local_player >= 0 {
            Some(local_player as u32)
        } else {
            None
        };
        let selection_ids = current_local_selection(local_player);
        let target = pick_context_target_for_click(
            &click_region,
            local_player_u32,
            &selection_ids,
            ui.is_in_force_attack_mode(),
            window_query,
            ui,
        );

        let mut translated: Vec<GameMessageType> = Vec::new();
        if let Some(target_id) = target {
            if pending_command_accepts_object(pending.options)
                && pending_command_target_allowed(pending.options, local_player, target_id)
                && pending_command_selection_valid(
                    &pending,
                    local_player_u32,
                    &selection_ids,
                    target_id,
                    ui,
                )
            {
                if let Some(message) = pending_command_for_object(&pending, target_id, ui) {
                    translated.push(message);
                }
            }

            if translated.is_empty() && pending_command_accepts_position(pending.options) {
                if let Some(obj) = OBJECT_REGISTRY.get_object(target_id) {
                    if let Ok(obj_guard) = obj.read() {
                        let position = logic_to_message_coord(obj_guard.get_position());
                        if pending_command_position_valid(
                            &pending,
                            local_player_u32,
                            &selection_ids,
                            &position,
                            Some(target_id),
                            ui,
                        ) {
                            translated = pending_command_messages_for_position(
                                &pending,
                                position,
                                &selection_ids,
                                Some(target_id),
                                ui,
                            );
                        }
                    }
                }
            }
        } else if pending_command_accepts_position(pending.options)
            && pending_command_position_valid(
                &pending,
                local_player_u32,
                &selection_ids,
                &world,
                None,
                ui,
            )
        {
            translated =
                pending_command_messages_for_position(&pending, world, &selection_ids, None, ui);
        }

        for message in &translated {
            dispatch_translated_message(message);
        }

        // C++ GUICommandTranslator suppresses one alternate-mouse blank-click deselect
        // after completing a non-context GUI command.
        ui.set_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click(true);
        // Non-context GUI command clicks complete this mode even when target validation fails.
        self.clear_pending_gui_command_mode(ui);
        GameMessageDisposition::DestroyMessage
    }
}

impl Default for GUICommandTranslator {
    fn default() -> Self {
        Self::new()
    }
}

impl GameMessageTranslator for GUICommandTranslator {
    fn requires_receiving_window_context(&self) -> bool {
        true
    }
    fn translate_game_message(&mut self, _msg: &GameMessage) -> GameMessageDisposition {
        log::error!("GUI command translation requires receiving presentation context");
        GameMessageDisposition::KeepMessage
    }
    fn translate_game_message_with_context(
        &mut self,
        msg: &GameMessage,
        client: game_engine::common::message_stream::TranslatorClientObservation,
        dispatch: &mut game_engine::common::message_stream::WindowInputDispatchFn<'_>,
        emitted: &mut Vec<GameMessage>,
        pick_view: Option<&game_engine::common::message_stream::TranslatorPickObjectView>,
        client_ui: &mut dyn std::any::Any,
        drawable_owner: Option<&mut dyn std::any::Any>,
    ) -> GameMessageDisposition {
        let ui = &mut crate::message_stream::ReceivingUi::new_with_owner(
            client_ui,
            drawable_owner,
            pick_view,
            client.logic_frame,
            client.frame,
        )
        .with_emitter(emitted);

        if let Some(pending) = ui.get_pending_command() {
            if is_pending_gui_non_context_command(&pending) {
                match msg.get_type() {
                    // Consume raw left input while in pending GUI command mode so selection
                    // translators do not start click/drag selection.
                    GameMessageType::RawMouseLeftButtonDown(..)
                    | GameMessageType::RawMouseLeftButtonUp(..) => {
                        return GameMessageDisposition::DestroyMessage;
                    }
                    GameMessageType::MouseLeftClick(region, _)
                    | GameMessageType::MouseLeftDoubleClick(region, _) => {
                        let mut window_query =
                            |x, y, ui: &mut crate::message_stream::ReceivingUi<'_>| {
                                match ui.dispatch(dispatch, game_engine::common::message_stream::WindowInputDispatch::OpaqueWindowAtScreen { x, y }) {
                            Some(game_engine::common::message_stream::WindowInputDispatchResult::OpaqueWindow(blocked)) => blocked,
                            _ => panic!("receiving GUI query returned an invalid response"),
                        }
                            };
                        return self.handle_pending_non_context_gui_click(
                            region,
                            &mut window_query,
                            ui,
                        );
                    }
                    _ => {}
                }
            }
        }

        match msg.get_type() {
            GameMessageType::MetaToggleControlBar => {
                self.handle_toggle_control_bar(dispatch, ui);
                GameMessageDisposition::DestroyMessage
            }
            GameMessageType::MetaDiplomacy => {
                self.handle_toggle_diplomacy(dispatch, ui);
                GameMessageDisposition::DestroyMessage
            }
            GameMessageType::MetaOptions => {
                info!("Toggling quit/options menu");
                crate::message_stream::meta_event::dispatch_ui_input(
                    game_engine::common::message_stream::UiInputAction::ToggleQuitMenu,
                    dispatch,
                    ui,
                );
                GameMessageDisposition::DestroyMessage
            }
            _ => GameMessageDisposition::KeepMessage,
        }
    }
}
