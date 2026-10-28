//! Native ControlBar operations borrow the receiving bar and camera directly.
use super::*;

#[cfg(feature = "game_client")]
impl CnCGameEngine {
    pub(super) fn receiving_observer_roster(
        &self,
    ) -> Option<game_client::gui::control_bar::control_bar_observer::NativeObserverRoster> {
        let recorder = self.game_logic.replay_pending.recorder_owned();
        // A live match has not started a replay recording yet: C++ still has
        // its selected lobby GameInfo, while this Recorder's header remains
        // empty until start_recording consumes pending_lobby. Project only
        // the admitted slot/team facts needed by the observer UI from this
        // exact match. Playback without a native admission keeps its file
        // header as the source.
        let admitted_game_info = self
            .game_logic
            .initial_skirmish_admission
            .as_deref()
            .map(crate::skirmish_config::replay_admission::observer_game_info);
        let game_info = admitted_game_info
            .as_ref()
            .or(Some(recorder.get_game_info()));
        receiving_observer_roster_from_world(
            &self.game_logic,
            recorder.is_multiplayer_for_game_mode(match self.game_logic.game_mode() {
                crate::game_logic::GameMode::SinglePlayer => 0,
                crate::game_logic::GameMode::Skirmish => 2,
                crate::game_logic::GameMode::Replay => 3,
                crate::game_logic::GameMode::Shell => 4,
                crate::game_logic::GameMode::None => 6,
                crate::game_logic::GameMode::Internet => 5,
                _ => 1,
            }),
            game_info,
        )
    }
    pub(super) fn receiving_observer_look_at_player(&self) -> Option<i32> {
        let index = self.control_bar.observer_target_index_owned()?;
        let id = u32::try_from(index).ok()?;
        self.game_logic
            .get_player(id)
            .filter(|p| !p.is_observer)
            .map(|_| index)
    }
    pub(super) fn update_receiving_observer_control_bar(
        &mut self,
        delta: std::time::Duration,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let receiving_callback_observation = self.receiving_control_bar_callback_observation();
        let game_ending = self.game_logic.owned_script_game_ending();
        self.control_bar.set_receiving_upgrade_definitions(
            &self.game_logic.upgrade_definitions,
            self.game_logic
                .receiving_module_content
                .as_ref()
                .map(|content| content.action_definitions_shared()),
        );
        let upgrade_observer = self.game_logic.receiving_upgrade_tooltip_observer();
        let upgrade_source = game_client::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource::Receiving(&upgrade_observer);
        let window_manager_owner = self.game_client.window_manager_owner();
        if !receiving_local_player_is_observer(&self.game_logic) {
            // A regular ControlBar update does not require replay metadata.
            let result =
                self.game_client
                    .with_window_and_control_bar_effects(None, |effects, videos| {
                        effects
                            .admit_control_bar_callback_observation(receiving_callback_observation);
                        effects.with_manager(|manager, effects| {
                            self.control_bar.update_owned_receiving(
                                delta,
                                None,
                                upgrade_source,
                                game_ending,
                                &window_manager_owner,
                                videos,
                                manager,
                                effects,
                            )
                        })
                    });
            return result;
        }
        let roster = self
            .receiving_observer_roster()
            .ok_or("receiving Recorder metadata unavailable for native observer")?;
        let result =
            self.game_client
                .with_window_and_control_bar_effects(None, |effects, videos| {
                    effects.admit_control_bar_callback_observation(receiving_callback_observation);
                    effects.with_manager(|manager, effects| {
                        self.control_bar.update_owned_receiving(
                            delta,
                            Some(&roster),
                            upgrade_source,
                            game_ending,
                            &window_manager_owner,
                            videos,
                            manager,
                            effects,
                        )
                    })
                });
        result
    }
    fn control_bar_parent_handle(
        &self,
    ) -> Option<std::rc::Rc<std::cell::RefCell<game_client::gui::GameWindow>>> {
        self.game_client
            .window_manager_owner()
            .with_manager_ref(|manager| {
                manager.find_window_by_name(crate::gameplay_layout::CONTROL_BAR_PARENT_NAME)
            })
    }

    pub(super) fn update_owned_control_bar_animations(&mut self) {
        // Source Mouse/WindowXlat observes stationary hover each admitted UI
        // frame before ControlBar consumes and clears its show-layout flag.
        let replay = matches!(
            self.game_logic.game_mode(),
            crate::game_logic::GameMode::Replay
        );
        let game_ending = self.game_logic.owned_script_game_ending();
        let tooltips_disabled = self
            .game_client
            .in_game_ui_subsystem()
            .map(|ui| ui.tooltips_disabled_at(self.game_logic.frame))
            .unwrap_or(true);
        let (x, y) = self.mouse_position;
        let observer_roster = self.receiving_observer_roster();
        let upgrade_tooltip = self.game_logic.receiving_upgrade_tooltip_observer();
        let menu_world = self.receiving_menu_world_facts();
        let control_bar_observation = self.receiving_control_bar_callback_observation();
        self.menu_audio
            .require_bound()
            .expect("native input admission requires initialized menu audio");
        let menu_audio_locality = self.game_logic.receiving_audio_locality_for_player(-1);
        let menu_audio_view = super::audio_view::CompletedAudioView::capture(&self.game_logic);
        let mut menu_audio = self.menu_audio.lend(
            &mut self.game_logic.session_view.audio_random,
            self.game_logic.queued_audio_events.policy(),
            menu_audio_locality,
            menu_audio_view,
        );
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
            input_provenance: game_client::gui::control_bar::HostControlBarInputProvenance::InjectedOrUnknown,
            ui, logic_frame: Some(menu_world.logic_frame), client_frame: Some(menu_world.client_frame),
            menus: Some(menus.reborrow()),
            ime:Some(ime),
            upgrade_tooltip: game_client::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource::Receiving(&upgrade_tooltip),
            replay_menu: None,
            replay_version: None,
            observer_roster: observer_roster.as_ref(),
            camera_options: None,
            control_bar: &mut self.control_bar,
            viewport: &mut self.game_logic.session_view.camera,
            replay,
            game_ending,
            tooltips_disabled,
            cursor_mouse,
            emitted_messages: None,
            campaign_manager: Some(campaign_manager),
            main_menu: Some(main_menu),
            window_manager_owner: Some(receiving_window_manager),
            menu_world: Some(menu_world),
            menu_host: Some(menu_host),
            script_display: Some(display),
            window_videos: Some(&mut videos.challenge),
            in_game_movies: Some(&mut videos.in_game),
        };
        receiving_window_manager.with_manager(|manager| {
            manager.refresh_control_bar_tooltip(x as i32, y as i32, &mut context);
            context
                .control_bar
                .update_build_tooltip_layout_owned(game_ending, manager);
            let parent =
                manager.find_window_by_name(crate::gameplay_layout::CONTROL_BAR_PARENT_NAME);
            context.update_control_bar_animations(parent, manager);
        });
    }

    pub(super) fn set_owned_control_bar_scheme(&mut self, side: &str) {
        let receiving_callback_observation = self.receiving_control_bar_callback_observation();
        let parent = self.control_bar_parent_handle();
        let replay = matches!(
            self.game_logic.game_mode(),
            crate::game_logic::GameMode::Replay
        );
        let side = if replay { "Observer" } else { side };
        let observer = receiving_local_player_is_observer(&self.game_logic);
        self.game_client.with_window_event_effects(None, |effects| {
            effects.admit_control_bar_callback_observation(receiving_callback_observation);
            effects.with_manager(|manager, effects| {
                self.control_bar
                    .set_control_bar_scheme_by_player_side_owned(
                        side,
                        &mut self.game_logic.session_view.camera,
                        replay,
                        observer,
                        parent,
                        manager,
                        effects,
                    )
            })
        });
    }

    pub(super) fn show_owned_control_bar(&mut self, immediate: bool) {
        let receiving_callback_observation = self.receiving_control_bar_callback_observation();
        let game_ending = self.game_logic.owned_script_game_ending();
        self.control_bar.set_receiving_upgrade_definitions(
            &self.game_logic.upgrade_definitions,
            self.game_logic
                .receiving_module_content
                .as_ref()
                .map(|content| content.action_definitions_shared()),
        );
        let upgrade_observer = self.game_logic.receiving_upgrade_tooltip_observer();
        let upgrade_source = game_client::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource::Receiving(&upgrade_observer);
        let replay = matches!(
            self.game_logic.game_mode(),
            crate::game_logic::GameMode::Replay
        );
        let parent = self
            .game_client
            .window_manager_owner()
            .with_manager(|manager| {
                game_client::gui::callbacks::replay_controls::show_replay_controls_owned(
                    manager, replay,
                );
                manager.find_window_by_name(crate::gameplay_layout::CONTROL_BAR_PARENT_NAME)
            });
        self.game_client.with_window_event_effects(None, |effects| {
            effects.admit_control_bar_callback_observation(receiving_callback_observation);
            effects.with_manager(|manager, effects| {
                game_client::gui::callbacks::control_bar_callbacks::show_control_bar_receiving(
                    &mut self.control_bar,
                    &mut self.game_logic.session_view.camera,
                    parent,
                    replay,
                    immediate,
                    upgrade_source,
                    game_ending,
                    manager,
                    effects,
                )
            })
        });
    }

    pub(super) fn hide_owned_control_bar(&mut self, immediate: bool) {
        let receiving_callback_observation = self.receiving_control_bar_callback_observation();
        let replay = matches!(
            self.game_logic.game_mode(),
            crate::game_logic::GameMode::Replay
        );
        let parent = self
            .game_client
            .window_manager_owner()
            .with_manager(|manager| {
                game_client::gui::callbacks::replay_controls::hide_replay_controls_owned(
                    manager, replay,
                );
                manager.find_window_by_name(crate::gameplay_layout::CONTROL_BAR_PARENT_NAME)
            });
        self.game_client.with_window_event_effects(None, |effects| {
            effects.admit_control_bar_callback_observation(receiving_callback_observation);
            effects.with_manager(|manager, effects| {
                game_client::gui::callbacks::control_bar_callbacks::hide_control_bar_owned(
                    &mut self.control_bar,
                    &mut self.game_logic.session_view.camera,
                    parent,
                    true,
                    immediate,
                    manager,
                    effects,
                )
            })
        });
    }

    pub(super) fn toggle_owned_control_bar(&mut self, immediate: bool) {
        let receiving_callback_observation = self.receiving_control_bar_callback_observation();
        let game_ending = self.game_logic.owned_script_game_ending();
        self.control_bar.set_receiving_upgrade_definitions(
            &self.game_logic.upgrade_definitions,
            self.game_logic
                .receiving_module_content
                .as_ref()
                .map(|content| content.action_definitions_shared()),
        );
        let upgrade_observer = self.game_logic.receiving_upgrade_tooltip_observer();
        let upgrade_source = game_client::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource::Receiving(&upgrade_observer);
        let replay = matches!(
            self.game_logic.game_mode(),
            crate::game_logic::GameMode::Replay
        );
        let parent = self
            .game_client
            .window_manager_owner()
            .with_manager(|manager| {
                game_client::gui::callbacks::replay_controls::toggle_replay_controls_owned(
                    manager, replay,
                );
                manager.find_window_by_name(crate::gameplay_layout::CONTROL_BAR_PARENT_NAME)
            });
        self.game_client.with_window_event_effects(None, |effects| {
            effects.admit_control_bar_callback_observation(receiving_callback_observation);
            effects.with_manager(|manager, effects| {
                game_client::gui::callbacks::control_bar_callbacks::toggle_control_bar_receiving(
                    &mut self.control_bar,
                    &mut self.game_logic.session_view.camera,
                    parent,
                    replay,
                    immediate,
                    upgrade_source,
                    game_ending,
                    manager,
                    effects,
                )
            })
        });
    }
}

/// Native script actions borrow UI for one synchronous interpreter operation.
/// GameLogic retains only its camera; no UI service is published into a world.
#[cfg(feature = "game_client")]
pub(super) struct NativeScriptUi<'a> {
    menu_audio_receipts: Option<&'a super::menu_audio::NativeMenuAudioService>,
    pub(super) input: Option<super::mouse::CinematicInputLoan<'a>>,
    window_manager_owner: game_client::gui::window_manager::WindowManagerOwner,
    snow: &'a mut game_client::snow::SnowManager,
    bar: &'a mut game_client::gui::control_bar::ControlBar,
    campaign_manager: Option<&'a mut game_client::gui::campaign_manager::CampaignManager>,
    command_hints: Option<&'a mut game_client::gui::command_hints::CommandHintState>,
    command_hint_frame: Option<u32>,
    window_effects: Option<game_client::gui::game_window::WindowEventEffects<'a>>,
    message_stream: Option<&'a mut game_client::message_stream::message_stream::MessageStream>,
}

#[cfg(feature = "game_client")]
impl<'a> NativeScriptUi<'a> {
    pub(super) fn with_menu_audio_receipts(
        mut self,
        audio: &'a super::menu_audio::NativeMenuAudioService,
    ) -> Self {
        self.menu_audio_receipts = Some(audio);
        self
    }

    pub(super) fn new(
        snow: &'a mut game_client::snow::SnowManager,
        bar: &'a mut game_client::gui::control_bar::ControlBar,
        window_manager_owner: game_client::gui::window_manager::WindowManagerOwner,
    ) -> Self {
        Self {
            input: None,
            menu_audio_receipts: None,
            window_manager_owner,
            snow,
            bar,
            campaign_manager: None,
            command_hints: None,
            command_hint_frame: None,
            window_effects: None,
            message_stream: None,
        }
    }

    pub(super) fn with_window_effects(
        mut self,
        effects: game_client::gui::game_window::WindowEventEffects<'a>,
        stream: &'a mut game_client::message_stream::message_stream::MessageStream,
    ) -> Self {
        self.window_effects = Some(effects);
        self.message_stream = Some(stream);
        self
    }

    pub(super) fn with_command_hints(
        mut self,
        hints: &'a mut game_client::gui::command_hints::CommandHintState,
        frame: u32,
    ) -> Self {
        self.command_hints = Some(hints);
        self.command_hint_frame = Some(frame);
        self
    }
    pub(super) fn with_campaign_manager(
        mut self,
        campaign_manager: &'a mut game_client::gui::campaign_manager::CampaignManager,
    ) -> Self {
        self.campaign_manager = Some(campaign_manager);
        self
    }

    pub(super) fn with_cinematic_input(
        mut self,
        input: super::mouse::CinematicInputLoan<'a>,
    ) -> Self {
        self.input = Some(input);
        self
    }
}

#[cfg(feature = "game_client")]
impl crate::game_logic::ScriptUiEffects for NativeScriptUi<'_> {
    fn visit_owned_menu_audio_receipts(
        &self,
        visit: &mut dyn FnMut(
            &std::sync::Arc<std::sync::Mutex<game_engine::common::audio::AudioManager>>,
            &[u32],
        ),
    ) {
        if let Some(audio) = self.menu_audio_receipts {
            audio.visit_owned_receipts(visit);
        }
    }

    fn observe_named_timer_counters(&mut self, counters: std::collections::HashMap<String, i32>) {
        self.window_effects
            .as_mut()
            .expect("native timer observation requires receiving callback services")
            .ui_mut()
            .observe_named_timer_counters(counters);
    }
    fn add_named_timer(&mut self, name: &str, text: &str, countdown: bool) {
        self.window_effects
            .as_mut()
            .expect("native timer action requires receiving callback services")
            .ui_mut()
            .add_named_timer(name, text, countdown);
    }
    fn remove_named_timer(&mut self, name: &str) {
        self.window_effects
            .as_mut()
            .expect("native timer action requires receiving callback services")
            .ui_mut()
            .remove_named_timer(name);
    }
    fn show_named_timer_display(&mut self, show: bool) {
        self.window_effects
            .as_mut()
            .expect("native timer action requires receiving callback services")
            .ui_mut()
            .show_named_timer_display(show);
    }
    fn hud_message(&mut self, text: &str, logic_frame: u32) {
        let effects = self
            .window_effects
            .as_mut()
            .expect("native HUD feedback requires receiving callback services");
        effects
            .ui_mut()
            .push_hud_message_at(text.to_owned(), logic_frame);
    }
    fn placement_preview_pending(&self) -> bool {
        let effects = self
            .window_effects
            .as_ref()
            .expect("native placement query requires receiving callback services");
        let ui = effects.ui();
        ui.get_pending_place_template().is_some() || ui.get_pending_place_source_object_id() != 0
    }
    fn admit_command_hint_client_frame(&mut self, frame: Option<u32>) {
        self.command_hint_frame = frame;
    }
    fn command_hint(&mut self, attack: bool, position: glam::Vec3, source: u32, eligible: bool) {
        let (Some(hints), Some(frame)) =
            (self.command_hints.as_deref_mut(), self.command_hint_frame)
        else {
            return;
        };
        let kind = if attack {
            game_client::gui::command_hints::CommandHintKind::Attack
        } else {
            game_client::gui::command_hints::CommandHintKind::Move
        };
        hints.record(kind, position, source, frame, eligible);
    }

    fn set_campaign_victorious(&mut self, victorious: bool) -> gamelogic::GameLogicResult<()> {
        let manager = match self.window_effects.as_mut() {
            Some(effects) => effects.receiving_campaign_manager_mut(),
            None => self.campaign_manager.as_deref_mut(),
        }
        .ok_or_else(|| {
            gamelogic::GameLogicError::Configuration(
                "native script action has no receiving CampaignManager".into(),
            )
        })?;
        manager.set_victorious(victorious);
        Ok(())
    }
    fn mark_control_bar_dirty(&mut self) {
        self.bar.mark_ui_dirty();
    }
    fn has_cinematic_input(&self) -> bool {
        self.input.is_some()
            && self
                .window_effects
                .as_ref()
                .is_some_and(|effects| effects.has_ui())
    }
    fn cinematic_input(
        &mut self,
        enabled: bool,
        was_enabled: bool,
        deselect: &mut dyn FnMut(u32),
    ) -> gamelogic::GameLogicResult<()> {
        let effects = self.window_effects.as_mut().ok_or_else(|| {
            gamelogic::GameLogicError::Configuration(
                "native cinematic input requires receiving callback services".into(),
            )
        })?;
        self.input
            .as_mut()
            .ok_or_else(|| {
                gamelogic::GameLogicError::Configuration(
                    "native cinematic input loan is absent".into(),
                )
            })?
            .apply(enabled, was_enabled, deselect, self.bar, effects)
    }

    fn weather_visible(&mut self, visible: bool) {
        self.snow.set_visible(visible);
    }

    fn letterbox(
        &mut self,
        camera: &mut crate::presentation_frame::camera::SessionCamera,
        enabled: bool,
        replay: bool,
        upgrade_source: game_engine::common::thing::receiving_module_content::UpgradeTooltipSource<
            '_,
        >,
        game_ending: bool,
    ) {
        let parent = self.window_manager_owner.with_manager(|manager| {
            if enabled {
                game_client::gui::callbacks::replay_controls::hide_replay_controls_owned(
                    manager, replay,
                );
            } else {
                game_client::gui::callbacks::replay_controls::show_replay_controls_owned(
                    manager, replay,
                );
            }
            manager.find_window_by_name(crate::gameplay_layout::CONTROL_BAR_PARENT_NAME)
        });
        let effects = self
            .window_effects
            .as_mut()
            .expect("native letterbox requires receiving callback services");
        if let Some(mut observation) = effects.control_bar_callback_observation() {
            observation.game_ending = game_ending;
            effects.admit_control_bar_callback_observation(observation);
        }
        let bar = &mut *self.bar;
        effects.with_manager(|manager, effects| {
            if enabled {
                game_client::gui::callbacks::control_bar_callbacks::hide_control_bar_owned(
                    bar, camera, parent, true, true, manager, effects,
                );
            } else {
                game_client::gui::callbacks::control_bar_callbacks::show_control_bar_receiving(
                    bar,
                    camera,
                    parent,
                    replay,
                    false,
                    upgrade_source,
                    game_ending,
                    manager,
                    effects,
                );
            }
        });
    }

    fn create_win_lose_window(&mut self, layout_filename: &str) -> gamelogic::GameLogicResult<()> {
        let effects = self.window_effects.as_mut().ok_or_else(|| {
            gamelogic::GameLogicError::Configuration(
                "native script window creation requires receiving callback services".into(),
            )
        })?;
        effects.with_manager(|manager, effects| {
            manager.create_script_message_window_with_effects(layout_filename, effects)
        });
        self.flush_window_callbacks()
    }
    fn destroy_win_lose_window(&mut self) -> gamelogic::GameLogicResult<()> {
        let effects = self.window_effects.as_mut().ok_or_else(|| {
            gamelogic::GameLogicError::Configuration(
                "native script window destruction requires receiving callback services".into(),
            )
        })?;
        effects.with_manager(|manager, effects| {
            manager.destroy_script_message_window_with_effects(effects)
        });
        self.flush_window_callbacks()
    }
    fn close_game_windows(&mut self) -> gamelogic::GameLogicResult<()> {
        let effects = self.window_effects.as_mut().ok_or_else(|| {
            gamelogic::GameLogicError::Configuration(
                "native closeWindows requires receiving callback services".into(),
            )
        })?;
        let bar = &mut *self.bar;
        effects.with_manager(|manager, effects| {
            game_client::core::game_client::close_receiving_game_windows(manager, bar, effects)
        });
        self.flush_window_callbacks()
    }
    fn popup_message(
        &mut self,
        popup: &crate::game_logic::ScriptPopupMessageRequest,
    ) -> gamelogic::GameLogicResult<()> {
        let effects = self.window_effects.as_mut().ok_or_else(|| {
            gamelogic::GameLogicError::Configuration(
                "native popup requires receiving client services".into(),
            )
        })?;
        game_client::gui::callbacks::in_game_popup_message::show_receiving_popup_message(
            &popup.message,
            popup.x_percent,
            popup.y_percent,
            popup.width,
            0xffffffff,
            popup.pause,
            popup.pause_music,
            Some(popup.popup_generation),
            effects,
        );
        self.flush_window_callbacks()
    }
    fn flush_window_callbacks(&mut self) -> gamelogic::GameLogicResult<()> {
        let Some(effects) = self.window_effects.as_mut() else {
            return Ok(());
        };
        let stream = self.message_stream.as_deref_mut().ok_or_else(|| {
            gamelogic::GameLogicError::Configuration(
                "native script callbacks require their receiving message stream".into(),
            )
        })?;
        effects.complete_callbacks_into(stream);

        Ok(())
    }
}

#[cfg(all(test, feature = "game_client"))]
#[path = "control_bar_viewport_tests.rs"]
mod script_ui_tests;

#[cfg(feature = "game_client")]
fn receiving_local_player_is_observer(world: &crate::game_logic::GameLogic) -> bool {
    world
        .local_player_id()
        .and_then(|id| world.get_player(id))
        .is_some_and(|player| player.is_observer)
}
#[cfg(feature = "game_client")]
fn receiving_observer_roster_from_world(
    world: &crate::game_logic::GameLogic,
    multiplayer: bool,
    game_info: Option<&game_engine::common::recorder::ReplayGameInfo>,
) -> Option<game_client::gui::control_bar::control_bar_observer::NativeObserverRoster> {
    use game_client::gui::control_bar::control_bar_observer::{
        NativeObserverPlayer, NativeObserverRoster,
    };
    let mut players: Vec<_> = world.get_players().values().collect();
    players.sort_by_key(|p| p.id);
    if multiplayer {
        // Source multiplayer walks named player0..player7, not arbitrary IDs or
        // the replay observer/neutral owners. The same receiving Main dict name
        // was admitted by the actual SidesList/GameSlot path.
        players = (0..8)
            .filter_map(|slot| {
                let name = format!("player{slot}");
                world
                    .get_players()
                    .values()
                    .find(|p| p.map_side.map_player_name == name)
            })
            .collect();
    } else if world.prepared_map_sides.get_num_sides() > 0 {
        // C++ getNthPlayer order is SidesList admission order. Main's player
        // map is keyed by host id, so recover that sequence from the admitted
        // side dicts before appending any non-side runtime players.
        let mut ordered = Vec::with_capacity(players.len());
        let mut admitted = std::collections::HashSet::new();
        for side_index in 0..world.prepared_map_sides.get_num_sides() {
            let Some(side) = world.prepared_map_sides.get_side_info(side_index) else {
                continue;
            };
            let name = side
                .get_dict()
                .get_ascii_string(game_engine::common::well_known_keys::key_player_name());
            if name.is_empty() {
                continue;
            }
            if let Some(player) = players.iter().copied().find(|player| {
                !admitted.contains(&player.id) && player.map_side.map_player_name == name
            }) {
                admitted.insert(player.id);
                ordered.push(player);
            }
        }
        ordered.extend(
            players
                .into_iter()
                .filter(|player| !admitted.contains(&player.id)),
        );
        players = ordered;
    }
    let mut compact_button = 0usize;
    Some(NativeObserverRoster {
        players: players
            .into_iter()
            .map(|p| {
                let template = world
                    .player_template_identity(p.id)
                    .and_then(|identity| world.resolve_player_template_identity(identity));
                let color = p.color_rgb;
                let count = |required: u128, forbidden: u128| {
                    world
                        .get_objects()
                        .registered_values()
                        .filter(|object| {
                            world.player_owner_for_host_object(object) == Some(p.id)
                                && object.is_kind_of_multi(required, forbidden)
                        })
                        .count() as i32
                };
                const SCORE: u128 = 1u128 << 35;
                const STRUCTURE: u128 = 1u128 << 7;
                const SCORE_CREATE: u128 = 1u128 << 36;
                const SCORE_DESTROY: u128 = 1u128 << 37;
                let team_key = if multiplayer && !p.is_observer {
                    let compact_index = compact_button;
                    compact_button += 1;
                    game_info
                        .and_then(|info| info.get_slot(compact_index))
                        .map(|slot| {
                            if slot.is_occupied && !slot.is_human && slot.team == -1 {
                                "Team:AI".to_string()
                            } else {
                                format!("Team:{}", slot.team + 1)
                            }
                        })
                } else {
                    None
                };
                Some(NativeObserverPlayer {
                    index: i32::try_from(p.id).ok()?,
                    slot_index: p
                        .map_side
                        .map_player_name
                        .strip_prefix("player")
                        .and_then(|slot| slot.parse::<i32>().ok())
                        .unwrap_or_else(|| i32::try_from(p.id).unwrap_or(i32::MAX)),
                    team_key,
                    observer: p.is_observer,
                    human: p.is_human,
                    display_name: p.name.clone(),
                    color_rgba: 0xFF00_0000
                        | ((color.0 as u32) << 16)
                        | ((color.1 as u32) << 8)
                        | color.2 as u32,
                    enabled_image: template
                        .as_ref()
                        .map(|template| template.get_enabled_image().to_string())
                        .unwrap_or_default(),
                    flag_water_mark: template
                        .as_ref()
                        .map(|template| template.get_flag_water_mark().to_string())
                        .unwrap_or_default(),
                    money: p.resources.supplies as i32,
                    power_produced: p.energy_production_at(world.frame),
                    power_consumed: p.power_consumed,
                    num_units: count(SCORE, STRUCTURE),
                    num_buildings: count(SCORE | STRUCTURE, 0)
                        + count(SCORE_CREATE | STRUCTURE, 0)
                        + count(SCORE_DESTROY | STRUCTURE, 0),
                    units_killed: p.statistics.score_keeper().get_total_units_destroyed(),
                    units_lost: p.statistics.score_keeper().get_total_units_lost(),
                })
            })
            .collect::<Option<Vec<_>>>()?,
        multiplayer,
        frame: world.frame,
    })
}

#[cfg(not(feature = "game_client"))]
impl CnCGameEngine {
    pub(super) fn receiving_observer_look_at_player(&self) -> Option<i32> {
        None
    }
}

#[cfg(all(test, feature = "game_client"))]
mod receiving_observer_roster_tests {
    use super::*;
    #[test]
    fn actual_same_id_world_players_drive_native_observer_roster_and_target() {
        use crate::game_logic::{GameLogic, Player, Team};
        let mut owner = GameLogic::new();
        let mut peer = GameLogic::new();
        for (world, name) in [(&mut owner, "Receiving"), (&mut peer, "Foreign")] {
            let mut ai = Player::new(0, Team::USA, "AI", false);
            ai.is_human = false;
            let mut observer = Player::new(1, Team::USA, "Observer", true);
            observer.is_observer = true;
            let mut human = Player::new(2, Team::USA, name, true);
            human.is_human = true;
            human.map_side.map_player_name = "player0".into();
            human.resources.supplies = if name == "Receiving" { 500 } else { 900 };
            let mut infantry_score = game_engine::common::rts::score_keeper::KindOfMaskType::new();
            infantry_score.set(game_engine::common::rts::score_keeper::KindOf::Infantry);
            human
                .statistics
                .record_destroyed("FixtureInfantry", &infantry_score, 0, false);
            if name == "Foreign" {
                human
                    .statistics
                    .record_destroyed("FixtureInfantry2", &infantry_score, 0, false);
            }
            world.add_player(ai);
            world.add_player(observer);
            world.add_player(human);
        }
        let roster = receiving_observer_roster_from_world(&owner, false, None).unwrap();
        let other = receiving_observer_roster_from_world(&peer, false, None).unwrap();
        assert_eq!(
            roster
                .selectable()
                .map(|p| (p.index, p.display_name.as_str()))
                .collect::<Vec<_>>(),
            vec![(2, "Receiving")]
        );
        assert_eq!(
            other
                .selectable()
                .map(|p| (p.index, p.display_name.as_str()))
                .collect::<Vec<_>>(),
            vec![(2, "Foreign")]
        );
        let receiving_player = roster.players.iter().find(|p| p.index == 2).unwrap();
        let foreign_player = other.players.iter().find(|p| p.index == 2).unwrap();
        assert_eq!(receiving_player.money, 500);
        assert_eq!(foreign_player.money, 900);
        assert_eq!(receiving_player.units_killed, 1);
        assert_eq!(foreign_player.units_killed, 2);
        let mut bar = game_client::gui::control_bar::ControlBar::new();
        bar.set_observer_mode(true);
        bar.admit_observer_button_owned(Some(2), &roster);
        peer.get_player_mut(2).unwrap().is_observer = true;
        let peer_changed = receiving_observer_roster_from_world(&peer, false, None).unwrap();
        assert!(peer_changed.selectable().next().is_none());
        assert_eq!(bar.observer_target_owned(&roster), Some(2));
        game_client::gui::control_bar::control_bar_observer::set_observer_look_at_player(Some(999));
        bar.update_owned_with_observers(std::time::Duration::from_millis(33), &roster)
            .unwrap();
        assert_eq!(bar.observer_target_owned(&roster), Some(2));
        assert_eq!(
            game_client::gui::control_bar::control_bar_observer::observer_look_at_player_index(),
            Some(999)
        );
        game_client::gui::control_bar::control_bar_observer::set_observer_look_at_player(None);
        let multi = receiving_observer_roster_from_world(&owner, true, None).unwrap();
        assert_eq!(
            multi.selectable().map(|p| p.index).collect::<Vec<_>>(),
            vec![2]
        );
        bar.clear_observer_target_owned();
        assert_eq!(bar.observer_target_owned(&roster), None);
    }

    #[test]
    fn native_observer_roster_preserves_sides_order_and_cpp_compact_game_info_team_slot() {
        use crate::game_logic::{GameLogic, Player, Team};
        use game_engine::common::dict::Dict;
        use game_engine::common::recorder::ReplayGameInfo;
        use game_engine::common::well_known_keys::key_player_name;

        let mut world = GameLogic::new();
        let mut second = Player::new(12, Team::USA, "Second", true);
        second.is_human = true;
        second.map_side.map_player_name = "SecondSide".into();
        let mut first = Player::new(2, Team::USA, "First", true);
        first.is_human = true;
        first.map_side.map_player_name = "FirstSide".into();
        world.add_player(first);
        world.add_player(second);

        let mut second_side = Dict::new();
        second_side.set_ascii_string(key_player_name(), "SecondSide");
        let mut first_side = Dict::new();
        first_side.set_ascii_string(key_player_name(), "FirstSide");
        world.prepared_map_sides.add_side(&second_side);
        world.prepared_map_sides.add_side(&first_side);
        let single_player = receiving_observer_roster_from_world(&world, false, None).unwrap();
        assert_eq!(
            single_player
                .selectable()
                .map(|player| player.index)
                .collect::<Vec<_>>(),
            vec![12],
            "C++ getNthPlayer order follows SidesList admission, not host id"
        );

        let mut observer = Player::new(0, Team::Neutral, "ReplayObserver", false);
        observer.is_observer = true;
        observer.map_side.map_player_name = "player0".into();
        let mut multiplayer_player = Player::new(1, Team::USA, "Recorded", true);
        multiplayer_player.is_human = true;
        multiplayer_player.map_side.map_player_name = "player1".into();
        world.add_player(observer);
        world.add_player(multiplayer_player);
        let mut game_info = ReplayGameInfo::new();
        let first_slot = game_info.get_slot_mut(0).unwrap();
        first_slot.is_occupied = true;
        first_slot.is_human = false;
        first_slot.team = -1;
        let second_slot = game_info.get_slot_mut(1).unwrap();
        second_slot.is_occupied = true;
        second_slot.is_human = true;
        second_slot.team = 4;
        let multiplayer =
            receiving_observer_roster_from_world(&world, true, Some(&game_info)).unwrap();
        let selected = multiplayer.selectable().next().unwrap();
        assert_eq!(selected.index, 1);
        assert_eq!(
            selected.team_key.as_deref(),
            Some("Team:AI"),
            "C++ uses compact currentButton to read GameInfo slot team, even after skipping player0"
        );
    }
}

#[cfg(all(test, feature = "game_client"))]
mod native_observer_local_identity_tests {
    use super::*;
    #[test]
    fn receiving_observer_mode_uses_actual_local_player_not_engine_command_player_zero() {
        use crate::game_logic::{GameLogic, Player, Team};
        let mut world = GameLogic::new();
        let mut command = Player::new(0, Team::USA, "RecordedPlayer", true);
        command.is_local = false;
        world.add_player(command);
        let mut observer = Player::new(17, Team::Neutral, "ReplayObserver", false);
        observer.is_local = true;
        observer.is_observer = true;
        world.add_player(observer);
        assert_eq!(world.local_player_id(), Some(17));
        assert!(!world.get_player(0).unwrap().is_observer);
        assert!(receiving_local_player_is_observer(&world));
        world.get_player_mut(17).unwrap().is_local = false;
        world.get_player_mut(0).unwrap().is_local = true;
        assert!(!receiving_local_player_is_observer(&world));
    }
}
