use super::*;
use gamelogic::helpers::TheAudio;

impl CommandTranslator {
    pub fn translate_with_window_query(
        &mut self,
        msg: &GameMessage,
        window_query: &mut dyn FnMut(i32, i32, &mut crate::message_stream::ReceivingUi<'_>) -> bool,
        ui: &mut crate::message_stream::ReceivingUi<'_>,
    ) -> GameMessageDisposition {
        // C++ CommandXlat.cpp:2322-2326 — DISABLE_INPUT destroys every message
        // except OPTIONS, CLEAR_GAME_DATA, and system ticks/CRC/replay camera.
        let msg_type = msg.get_type();
        let input_disabled_passthrough = matches!(
            msg_type,
            GameMessageType::MetaOptions
                | GameMessageType::ClearGameData
                | GameMessageType::DestroySelectedGroup(_)
                | GameMessageType::LogicCRC(_)
                | GameMessageType::SetReplayCamera(..)
                | GameMessageType::FrameTick(_)
        );
        if !ui.get_input_enabled() && !input_disabled_passthrough {
            return GameMessageDisposition::DestroyMessage;
        }

        let (new_messages, disposition) = match msg.get_type() {
            GameMessageType::FrameTick(_) => {
                // Keep the client-side selection cache in sync with the GameLogic selection manager
                // so commands work after selection changes originating outside this translator
                // (control groups, scripts, multiplayer, etc.).
                self.sync_selection_from_logic();
                return GameMessageDisposition::KeepMessage;
            }
            GameMessageType::CreateSelectedGroup(create_new, objects) => {
                if *create_new {
                    self.current_selection.clear();
                }
                self.current_selection.extend(objects.iter().copied());
                return GameMessageDisposition::KeepMessage;
            }
            GameMessageType::CreateSelectedGroupNoSound(create_new, objects) => {
                if *create_new {
                    self.current_selection.clear();
                }
                self.current_selection.extend(objects.iter().copied());
                return GameMessageDisposition::KeepMessage;
            }
            GameMessageType::RemoveFromSelectedGroup(objects) => {
                for object_id in objects {
                    self.current_selection.remove(object_id);
                }
                return GameMessageDisposition::KeepMessage;
            }
            GameMessageType::MetaBeginForceAttack => {
                self.force_attack_mode = true;
                ui.set_force_attack_mode(true);
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaEndForceAttack => {
                self.force_attack_mode = false;
                ui.set_force_attack_mode(false);
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaBeginForceMove => {
                self.force_move_mode = true;
                ui.set_force_move_to_mode(true);
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaEndForceMove => {
                self.force_move_mode = false;
                ui.set_force_move_to_mode(false);
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaBeginWaypoints => {
                self.waypoint_mode = true;
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaEndWaypoints => {
                self.waypoint_mode = false;
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaBeginPreferSelection => {
                self.prefer_selection_mode = true;
                ui.set_prefer_selection_mode(true);
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaEndPreferSelection => {
                self.prefer_selection_mode = false;
                ui.set_prefer_selection_mode(false);
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaBeginPathBuild => {
                dispatch_translated_message(&GameMessageType::MetaBeginPathBuild);
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaEndPathBuild => {
                dispatch_translated_message(&GameMessageType::MetaEndPathBuild);
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaStop => {
                ui.issue_stop_command();
                dispatch_translated_message(&GameMessageType::DoStop);
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaScatter => {
                dispatch_translated_message(&GameMessageType::DoScatter);
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaCreateFormation => {
                dispatch_translated_message(&GameMessageType::CreateFormation(Vec::new()));
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaViewLastRadarEvent => {
                if let Some(position) = ui
                    .radar_presentation()
                    .and_then(|radar| radar.get_last_event_loc())
                {
                    with_tactical_view(|view| {
                        view.look_at(&Point3::new(position.x, position.y, position.z));
                    });
                }
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaAllCheer => {
                if TheGameLogic::is_in_multiplayer_game() {
                    // C++ CommandXlat.cpp:3468-3476 — play MiscAudio AllCheerSound
                    // before appending MSG_DO_CHEER.
                    if let Some(audio) = TheAudio::get() {
                        let _ = audio.add_audio_event(&TheAudio::get_misc_audio().all_cheer_sound);
                    }
                    dispatch_translated_message(&GameMessageType::DoCheer);
                }
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaToggleFastForwardReplay => {
                if TheGameLogic::is_in_replay_game() {
                    if let Some(global_data) = get_global_data() {
                        let enabled = {
                            let mut guard = global_data.write();
                            guard.tivo_fast_mode = !guard.tivo_fast_mode;
                            guard.tivo_fast_mode
                        };
                        ui.message(if enabled {
                            "m_TiVOFastMode: ON"
                        } else {
                            "m_TiVOFastMode: OFF"
                        });
                    }
                }
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaDemoInstantQuit => {
                if TheGameLogic::is_in_game() {
                    let _ = TheGameLogic::clear_game_data();
                }
                if let Some(engine) = get_game_engine() {
                    engine.lock().set_quitting(true);
                }
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaSelectNextUnit => {
                return {
                    for msg in handle_select_next_or_prev_unit(true) {
                        dispatch_translated_message(&msg);
                    }
                    GameMessageDisposition::DestroyMessage
                };
            }
            GameMessageType::MetaSelectPrevUnit => {
                return {
                    for msg in handle_select_next_or_prev_unit(false) {
                        dispatch_translated_message(&msg);
                    }
                    GameMessageDisposition::DestroyMessage
                };
            }
            GameMessageType::MetaSelectNextWorker => {
                return {
                    for msg in handle_select_next_or_prev_worker(true) {
                        dispatch_translated_message(&msg);
                    }
                    GameMessageDisposition::DestroyMessage
                };
            }
            GameMessageType::MetaSelectPrevWorker => {
                return {
                    for msg in handle_select_next_or_prev_worker(false) {
                        dispatch_translated_message(&msg);
                    }
                    GameMessageDisposition::DestroyMessage
                };
            }
            GameMessageType::MetaSelectAll => {
                return {
                    for msg in handle_select_all(false, window_query, ui) {
                        dispatch_translated_message(&msg);
                    }
                    GameMessageDisposition::DestroyMessage
                };
            }
            GameMessageType::MetaSelectAllAircraft => {
                return {
                    for msg in handle_select_all(true, window_query, ui) {
                        dispatch_translated_message(&msg);
                    }
                    GameMessageDisposition::DestroyMessage
                };
            }
            GameMessageType::MetaDeploy
            | GameMessageType::MetaFollow
            | GameMessageType::MetaChatPlayers
            | GameMessageType::MetaChatAllies
            | GameMessageType::MetaChatEveryone => {
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MouseRightClick(region, _modifiers) => (
                self.handle_point_click(region, true, window_query, ui),
                GameMessageDisposition::DestroyMessage,
            ),
            GameMessageType::MouseLeftClick(region, _modifiers) => (
                self.handle_point_click(region, false, window_query, ui),
                GameMessageDisposition::DestroyMessage,
            ),
            GameMessageType::MouseRightDoubleClick(region, _modifiers) => (
                self.try_double_click_guard_command(region, true, ui)
                    .map(|msg| vec![msg])
                    .unwrap_or_else(|| self.handle_point_click(region, true, window_query, ui)),
                GameMessageDisposition::DestroyMessage,
            ),
            GameMessageType::MouseLeftDoubleClick(region, _modifiers) => (
                self.try_double_click_guard_command(region, false, ui)
                    .map(|msg| vec![msg])
                    .unwrap_or_else(|| self.handle_point_click(region, false, window_query, ui)),
                GameMessageDisposition::DestroyMessage,
            ),
            GameMessageType::MouseoverLocationHint(pos) => (
                self.handle_mouseover_location_hint(pos, ui),
                GameMessageDisposition::KeepMessage,
            ),
            GameMessageType::MouseoverDrawableHint(drawable) => (
                self.handle_mouseover_drawable_hint(*drawable, ui),
                GameMessageDisposition::KeepMessage,
            ),
            GameMessageType::RawMouseLeftButtonDown(pos, _modifiers, time) => (
                self.handle_mouse_button_down(pos, MouseButton::Left, *_modifiers, *time),
                GameMessageDisposition::KeepMessage,
            ),
            GameMessageType::RawMouseRightButtonDown(pos, _modifiers, time) => (
                self.handle_mouse_button_down(pos, MouseButton::Right, *_modifiers, *time),
                GameMessageDisposition::KeepMessage,
            ),
            GameMessageType::RawMouseMiddleButtonDown(pos, _modifiers, time) => (
                self.handle_mouse_button_down(pos, MouseButton::Middle, *_modifiers, *time),
                GameMessageDisposition::DestroyMessage,
            ),
            GameMessageType::RawMouseLeftButtonUp(pos, _modifiers, time) => (
                self.handle_mouse_button_up(
                    pos,
                    MouseButton::Left,
                    *_modifiers,
                    *time,
                    window_query,
                    ui,
                ),
                GameMessageDisposition::KeepMessage,
            ),
            GameMessageType::RawMouseRightButtonUp(pos, _modifiers, time) => (
                self.handle_mouse_button_up(
                    pos,
                    MouseButton::Right,
                    *_modifiers,
                    *time,
                    window_query,
                    ui,
                ),
                GameMessageDisposition::KeepMessage,
            ),
            GameMessageType::RawMouseMiddleButtonUp(pos, _modifiers, time) => (
                self.handle_mouse_button_up(
                    pos,
                    MouseButton::Middle,
                    *_modifiers,
                    *time,
                    window_query,
                    ui,
                ),
                GameMessageDisposition::DestroyMessage,
            ),
            GameMessageType::RawKeyDown(key) => (
                self.handle_keyboard(*key, true, ui),
                GameMessageDisposition::KeepMessage,
            ),
            GameMessageType::RawKeyUp(key) => (
                self.handle_keyboard(*key, false, ui),
                GameMessageDisposition::KeepMessage,
            ),
            _ => {
                // Pass through other messages unchanged
                return GameMessageDisposition::KeepMessage;
            }
        };

        // Translated high-level messages are forwarded into the command list, matching the C++
        // message stream flow where raw input messages are consumed and replaced with commands.
        for new_msg in new_messages {
            dispatch_translated_message(&new_msg);
        }

        disposition
    }
}

impl GameMessageTranslator for CommandTranslator {
    fn requires_receiving_window_context(&self) -> bool {
        true
    }
    fn translate_game_message(&mut self, msg: &GameMessage) -> GameMessageDisposition {
        log::error!(
            "GUI translator requires translate_game_message_with_context; message left untranslated"
        );
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

        self.translate_with_window_query(
            msg,
            &mut |x, y, ui| match ui.dispatch(
                dispatch,
                game_engine::common::message_stream::WindowInputDispatch::OpaqueWindowAtScreen {
                    x,
                    y,
                },
            ) {
                Some(
                    game_engine::common::message_stream::WindowInputDispatchResult::OpaqueWindow(
                        blocked,
                    ),
                ) => blocked,
                _ => panic!("receiving input driver did not supply its opaque-window query"),
            },
            ui,
        )
    }
}
