//! Native ControlBar operations borrow the receiving bar and camera directly.
use super::*;

#[cfg(feature = "game_client")]
impl CnCGameEngine {
    pub(super) fn receiving_observer_roster(
        &self,
    ) -> Option<game_client::gui::control_bar::control_bar_observer::NativeObserverRoster> {
        receiving_observer_roster_from_world(
            &self.game_logic,
            self.game_logic
                .replay_pending
                .recorder_owned()
                .is_multiplayer_for_game_mode(match self.game_logic.game_mode() {
                    crate::game_logic::GameMode::SinglePlayer => 0,
                    crate::game_logic::GameMode::Skirmish => 2,
                    crate::game_logic::GameMode::Replay => 3,
                    crate::game_logic::GameMode::Shell => 4,
                    crate::game_logic::GameMode::None => 6,
                    crate::game_logic::GameMode::Internet => 5,
                    _ => 1,
                }),
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
        let game_ending = self.game_logic.owned_script_game_ending();
        let upgrade_observer = self.game_logic.receiving_upgrade_tooltip_observer();
        let upgrade_source = game_client::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource::Receiving(&upgrade_observer);
        let mut emitted_messages = Vec::new();
        let window_manager_owner = self.game_client.window_manager_owner();
        if !receiving_local_player_is_observer(&self.game_logic) {
            // A regular ControlBar update does not require replay metadata.
            let result = self.control_bar.update_owned_receiving(
                delta,
                None,
                upgrade_source,
                game_ending,
                &window_manager_owner,
                &mut emitted_messages,
            );
            for message in emitted_messages {
                self.game_client
                    .message_stream_mut()
                    .append_message_value(message);
            }
            return result;
        }
        let roster = self
            .receiving_observer_roster()
            .ok_or("receiving Recorder metadata unavailable for native observer")?;
        let result = self.control_bar.update_owned_receiving(
            delta,
            Some(&roster),
            upgrade_source,
            game_ending,
            &window_manager_owner,
            &mut emitted_messages,
        );
        for message in emitted_messages {
            self.game_client
                .message_stream_mut()
                .append_message_value(message);
        }
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
            .and_then(|ui| {
                ui.lock()
                    .ok()
                    .map(|ui| ui.tooltips_disabled_at(self.game_logic.frame))
            })
            .unwrap_or(true);
        let cursor_mouse = self.game_client.input_mouse_handle();
        let (x, y) = self.mouse_position;
        let observer_roster = self.receiving_observer_roster();
        let upgrade_tooltip = self.game_logic.receiving_upgrade_tooltip_observer();
        let mut context = game_client::gui::window_manager::ControlBarInputContext {
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
            cursor_mouse: cursor_mouse.as_ref(),
            emitted_messages: None,
            campaign_manager: None,
            window_manager_owner: None,
        };
        self.game_client
            .window_manager_owner()
            .with_manager(|manager| {
                manager.refresh_control_bar_tooltip(x as i32, y as i32, &mut context);
            });
        self.control_bar
            .update_build_tooltip_layout_owned(game_ending);
        let parent = self.control_bar_parent_handle();
        self.control_bar_callbacks.update_window_animations(parent);
    }

    pub(super) fn set_owned_control_bar_scheme(&mut self, side: &str) {
        let parent = self.control_bar_parent_handle();
        let replay = matches!(
            self.game_logic.game_mode(),
            crate::game_logic::GameMode::Replay
        );
        let side = if replay { "Observer" } else { side };
        let observer = receiving_local_player_is_observer(&self.game_logic);
        self.control_bar
            .set_control_bar_scheme_by_player_side_owned(
                side,
                &mut self.game_logic.session_view.camera,
                replay,
                observer,
                parent,
            );
    }

    pub(super) fn show_owned_control_bar(&mut self, immediate: bool) {
        let game_ending = self.game_logic.owned_script_game_ending();
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
        self.control_bar_callbacks.show_control_bar_receiving(
            &mut self.control_bar,
            &mut self.game_logic.session_view.camera,
            parent,
            replay,
            immediate,
            upgrade_source,
            game_ending,
        );
    }

    pub(super) fn hide_owned_control_bar(&mut self, immediate: bool) {
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
        self.control_bar_callbacks.hide_control_bar_owned(
            &mut self.control_bar,
            &mut self.game_logic.session_view.camera,
            parent,
            true,
            immediate,
        );
    }

    pub(super) fn toggle_owned_control_bar(&mut self, immediate: bool) {
        let game_ending = self.game_logic.owned_script_game_ending();
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
        self.control_bar_callbacks.toggle_control_bar_receiving(
            &mut self.control_bar,
            &mut self.game_logic.session_view.camera,
            parent,
            replay,
            immediate,
            upgrade_source,
            game_ending,
        );
    }
}

/// Native script actions borrow UI for one synchronous interpreter operation.
/// GameLogic retains only its camera; no UI service is published into a world.
#[cfg(feature = "game_client")]
pub(super) struct NativeScriptUi<'a> {
    pub(super) input: Option<super::mouse::CinematicInputLoan<'a>>,
    window_manager_owner: game_client::gui::window_manager::WindowManagerOwner,
    snow: &'a mut game_client::snow::SnowManager,
    bar: &'a mut game_client::gui::control_bar::ControlBar,
    callbacks: &'a mut game_client::gui::callbacks::control_bar_callbacks::ControlBarCallbacks,
    campaign_manager: Option<&'a mut game_client::gui::campaign_manager::CampaignManager>,
}

#[cfg(feature = "game_client")]
impl<'a> NativeScriptUi<'a> {
    pub(super) fn new(
        snow: &'a mut game_client::snow::SnowManager,
        bar: &'a mut game_client::gui::control_bar::ControlBar,
        callbacks: &'a mut game_client::gui::callbacks::control_bar_callbacks::ControlBarCallbacks,
        window_manager_owner: game_client::gui::window_manager::WindowManagerOwner,
    ) -> Self {
        Self {
            input: None,
            window_manager_owner,
            snow,
            bar,
            callbacks,
            campaign_manager: None,
        }
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

    fn letterbox_with_parent(
        &mut self,
        camera: &mut crate::presentation_frame::camera::SessionCamera,
        enabled: bool,
        replay: bool,
        parent: Option<std::rc::Rc<std::cell::RefCell<game_client::gui::GameWindow>>>,
        upgrade_source: game_client::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource<'_>,
        game_ending: bool,
    ) {
        // ScriptActions.cpp:3749-3760: HideControlBar(TRUE) precedes enabling
        // letterbox; ShowControlBar(FALSE) precedes disabling it. Every authored
        // call applies even if the display's letterbox flag is unchanged.
        if enabled {
            self.callbacks
                .hide_control_bar_owned(self.bar, camera, parent, true, true);
        } else {
            self.callbacks.show_control_bar_receiving(
                self.bar,
                camera,
                parent,
                replay,
                false,
                upgrade_source,
                game_ending,
            );
        }
    }
}

#[cfg(feature = "game_client")]
impl crate::game_logic::ScriptUiEffects for NativeScriptUi<'_> {
    fn set_campaign_victorious(&mut self, victorious: bool) -> gamelogic::GameLogicResult<()> {
        let manager = self.campaign_manager.as_deref_mut().ok_or_else(|| {
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
        self.input.as_ref().is_some_and(|input| input.available())
    }
    fn cinematic_input(
        &mut self,
        enabled: bool,
        was_enabled: bool,
        deselect: &mut dyn FnMut(u32),
    ) -> gamelogic::GameLogicResult<()> {
        self.input
            .as_mut()
            .ok_or_else(|| {
                gamelogic::GameLogicError::Configuration(
                    "native cinematic input loan is absent".into(),
                )
            })?
            .apply(enabled, was_enabled, deselect, self.bar)
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
        self.letterbox_with_parent(camera, enabled, replay, parent, upgrade_source, game_ending);
    }

    fn create_win_lose_window(&mut self, layout_filename: &str) -> gamelogic::GameLogicResult<()> {
        self.window_manager_owner
            .with_manager(|manager| manager.create_script_message_window(layout_filename));
        Ok(())
    }

    fn destroy_win_lose_window(&mut self) -> gamelogic::GameLogicResult<()> {
        self.window_manager_owner
            .with_manager(|manager| manager.destroy_script_message_window());
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
    }
    Some(NativeObserverRoster {
        players: players
            .into_iter()
            .map(|p| {
                Some(NativeObserverPlayer {
                    index: i32::try_from(p.id).ok()?,
                    observer: p.is_observer,
                    human: p.is_human,
                    display_name: p.name.clone(),
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
            world.add_player(ai);
            world.add_player(observer);
            world.add_player(human);
        }
        let roster = receiving_observer_roster_from_world(&owner, false).unwrap();
        let other = receiving_observer_roster_from_world(&peer, false).unwrap();
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
        let mut bar = game_client::gui::control_bar::ControlBar::new();
        bar.set_observer_mode(true);
        bar.admit_observer_button_owned(Some(2), &roster);
        peer.get_player_mut(2).unwrap().is_observer = true;
        let peer_changed = receiving_observer_roster_from_world(&peer, false).unwrap();
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
        let multi = receiving_observer_roster_from_world(&owner, true).unwrap();
        assert_eq!(
            multi.selectable().map(|p| p.index).collect::<Vec<_>>(),
            vec![2]
        );
        bar.clear_observer_target_owned();
        assert_eq!(bar.observer_target_owned(&roster), None);
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
