#[cfg(test)]
#[path = "fixture_owner.rs"]
mod fixture_owner;
// Split from `gui/load_screen.rs` dump. Included by `load_screen/mod.rs`.

#[cfg(test)]
mod tests {
    include!("tests/selection_and_metadata.rs");
    include!("tests/multiplayer_layout.rs");
    include!("tests/progress_and_input.rs");
    include!("tests/single_player_prelude.rs");
    include!("tests/challenge_prelude.rs");
    include!("tests/shell_legal_hold.rs");

    use super::fixture_owner::{AudioTrace, LoadScreenFixture, accepted_handle};
    use super::*;
    use crate::game_network::{GameInfo, GameSlot, SlotState};
    use crate::gui::gadgets::progressbar::ProgressBar;
    use crate::gui::game_window::WindowWidget;
    use game_engine::common::ini::ini_map_cache::{Coord3D, Region3D};
    use game_engine::common::language::Language;
    use std::cell::Cell;
    use std::rc::Rc;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };

    fn with_fixture_mouse<R>(
        runtime: &mut LoadScreenOperation<'_>,
        operation: impl FnOnce(&mut crate::input::Mouse) -> R,
    ) -> R {
        operation(
            runtime
                .effects
                .cursor_mouse_mut()
                .expect("fixture has actual receiving Mouse"),
        )
    }

    fn named_test_window(
        wm: &mut WindowManager,
        name: &str,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        let window = wm
            .create_window(None, 0, 0, 100, 20, effects)
            .expect("window");
        let mut window = window.borrow_mut();
        window.set_name(name);
        let _ = window.hide(true, wm);
    }

    fn named_progress_test_window(
        wm: &mut WindowManager,
        name: &str,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        let window = wm
            .create_window(None, 0, 0, 100, 20, effects)
            .expect("window");
        let mut window = window.borrow_mut();
        window.set_name(name);
        window.set_widget(WindowWidget::ProgressBar(ProgressBar::new(
            0, 0, 0, 100, 20,
        )));
        let _ = window.hide(true, wm);
    }

    fn create_multiplayer_slot_windows(
        wm: &mut WindowManager,
        prefix: &str,
        count: usize,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        for slot in 0..count {
            named_progress_test_window(wm, &format!("{prefix}:ProgressLoad{slot}"), effects);
            let suffixes = if load_screen_has_team_windows(prefix) {
                &["StaticTextPlayer", "StaticTextSide", "StaticTextTeam"][..]
            } else {
                &["StaticTextPlayer", "StaticTextSide"][..]
            };
            for suffix in suffixes {
                named_test_window(wm, &format!("{prefix}:{suffix}{slot}"), effects);
            }
        }
    }

    fn create_multiplayer_start_position_windows(
        wm: &mut WindowManager,
        prefix: &str,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        for slot in 0..MAX_LOAD_SCREEN_SLOTS {
            let name = format!("{prefix}:ButtonMapStartPosition{slot}");
            named_test_window(wm, &name, effects);
            wm.find_window_by_name(&name)
                .expect("start position button")
                .borrow_mut()
                .set_size(10, 10, effects)
                .expect("button size");
        }
    }

    fn create_gamespy_slot_windows(
        wm: &mut WindowManager,
        count: usize,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        for slot in 0..count {
            named_test_window(
                wm,
                &format!("GameSpyLoadScreen.wnd:WinPlayer{slot}"),
                effects,
            );
            for suffix in gamespy_stats_suffixes() {
                named_test_window(
                    wm,
                    &format!("GameSpyLoadScreen.wnd:{suffix}{slot}"),
                    effects,
                );
            }
        }
    }

    fn create_map_transfer_slot_windows(
        wm: &mut WindowManager,
        count: usize,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        named_test_window(wm, "MapTransferScreen.wnd:StaticTextCurrentFile", effects);
        named_test_window(wm, "MapTransferScreen.wnd:StaticTextTimeout", effects);
        for slot in 0..count {
            named_progress_test_window(
                wm,
                &format!("MapTransferScreen.wnd:ProgressLoad{slot}"),
                effects,
            );
            named_test_window(
                wm,
                &format!("MapTransferScreen.wnd:StaticTextPlayer{slot}"),
                effects,
            );
            named_test_window(
                wm,
                &format!("MapTransferScreen.wnd:StaticTextProgress{slot}"),
                effects,
            );
        }
    }

    fn load_screen_slot(
        player_name: &str,
        side_name: &str,
        team_number: i32,
        is_ai: bool,
        visible: bool,
    ) -> LoadScreenSlotInitContext {
        load_screen_slot_with_color(player_name, side_name, team_number, None, is_ai, visible)
    }

    fn load_screen_slot_with_color(
        player_name: &str,
        side_name: &str,
        team_number: i32,
        apparent_color: Option<i32>,
        is_ai: bool,
        visible: bool,
    ) -> LoadScreenSlotInitContext {
        load_screen_slot_with_text_color(
            player_name,
            side_name,
            team_number,
            apparent_color,
            None,
            is_ai,
            visible,
        )
    }

    fn load_screen_slot_with_text_color(
        player_name: &str,
        side_name: &str,
        team_number: i32,
        apparent_color: Option<i32>,
        apparent_text_color: Option<u32>,
        is_ai: bool,
        visible: bool,
    ) -> LoadScreenSlotInitContext {
        LoadScreenSlotInitContext {
            player_id: team_number,
            player_name: player_name.to_string(),
            side_name: side_name.to_string(),
            team_number,
            apparent_color,
            apparent_text_color,
            is_ai,
            has_map: true,
            visible,
        }
    }

    fn load_screen_slot_with_map(
        player_id: i32,
        player_name: &str,
        apparent_text_color: Option<u32>,
        has_map: bool,
        is_ai: bool,
        visible: bool,
    ) -> LoadScreenSlotInitContext {
        let mut slot = load_screen_slot_with_text_color(
            player_name,
            "USA",
            player_id,
            None,
            apparent_text_color,
            is_ai,
            visible,
        );
        slot.player_id = player_id;
        slot.has_map = has_map;
        slot
    }

    fn window_text(wm: &WindowManager, name: &str) -> String {
        wm.find_window_by_name(name)
            .expect(name)
            .borrow()
            .get_text()
            .to_string()
    }

    fn window_text_color(wm: &WindowManager, name: &str) -> u32 {
        wm.find_window_by_name(name)
            .expect(name)
            .borrow()
            .get_enabled_text_color()
    }

    fn window_enabled_image_name(wm: &WindowManager, name: &str, index: usize) -> Option<String> {
        wm.find_window_by_name(name)
            .expect(name)
            .borrow()
            .get_enabled_draw_data(index)
            .and_then(|draw| draw.image)
            .map(|image| image.name)
    }

    fn window_hidden(wm: &WindowManager, name: &str) -> bool {
        wm.find_window_by_name(name)
            .expect(name)
            .borrow()
            .is_hidden()
    }

    fn window_position(wm: &WindowManager, name: &str) -> (i32, i32) {
        wm.find_window_by_name(name)
            .expect(name)
            .borrow()
            .get_position()
    }

    fn window_image_name(wm: &WindowManager, name: &str, index: usize) -> Option<String> {
        wm.find_window_by_name(name)
            .expect(name)
            .borrow()
            .get_enabled_draw_data(index)?
            .image
            .map(|image| image.name)
    }

    fn window_status(wm: &WindowManager, name: &str) -> WindowStatus {
        wm.find_window_by_name(name)
            .expect(name)
            .borrow()
            .get_status()
    }

    fn progress_value(name: &str, runtime: &mut LoadScreenOperation<'_>) -> Option<f32> {
        runtime.windows.with_manager(|wm| {
            let window = wm.find_window_by_name(name)?;
            let mut window = window.borrow_mut();
            Some(window.progress_bar_mut()?.value())
        })
    }

    fn progress_fill_color(wm: &WindowManager, name: &str) -> crate::gui::gadgets::Color {
        wm.find_window_by_name(name)
            .expect(name)
            .borrow_mut()
            .progress_bar_mut()
            .expect("progress widget")
            .fill_color()
    }

    /// Invoke the actual source preparation and legal-hold phases around
    /// the canonical client's window owner; no manager is moved or selected.
    fn initialize_shell_fixture(
        mem_pass: bool,
        runtime: &mut LoadScreenOperation<'_>,
        pump: &mut LoadScreenPresentationPump<'_>,
    ) {
        let windows = runtime.windows;
        let hold =
            windows.with_manager(|wm| prepare_shell_game_windows(wm, mem_pass, runtime.first_load));
        if hold {
            run_shell_game_legal_hold_with_pump(Some(pump), runtime).unwrap();
            *runtime.first_load = false;
            windows
                .with_manager(|wm| hide_window(wm, "ShellGameLoadScreen.wnd:ProgressLoad", false));
        }
    }
    fn shell_fixture_windows(runtime: &mut LoadScreenOperation<'_>) {
        let windows = runtime.windows;
        windows.with_manager(|wm| {
            wm.destroy_all_windows(&mut runtime.effects);
            let root = wm
                .create_window(None, 0, 0, 800, 600, &mut runtime.effects)
                .expect("root");
            root.borrow_mut()
                .set_name("ShellGameLoadScreen.wnd:ParentShellGameLoadScreen");
            named_test_window(
                wm,
                "ShellGameLoadScreen.wnd:StaticTextLegal",
                &mut runtime.effects,
            );
            named_test_window(
                wm,
                "ShellGameLoadScreen.wnd:ProgressLoad",
                &mut runtime.effects,
            );
        });
    }

    fn challenge_test_windows(
        wm: &mut WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        named_test_window(
            wm,
            "ChallengeLoadScreen.wnd:ParentChallengeLoadScreen",
            effects,
        );
        for name in CHALLENGE_BIO_LABEL_WINDOWS
            .iter()
            .chain(CHALLENGE_BIO_ENTRY_WINDOWS.iter())
            .copied()
            .chain(
                [
                    "ChallengeLoadScreen.wnd:PortraitLeft",
                    "ChallengeLoadScreen.wnd:PortraitRight",
                    "ChallengeLoadScreen.wnd:PortraitMovieLeft",
                    "ChallengeLoadScreen.wnd:PortraitMovieRight",
                    "ChallengeLoadScreen.wnd:CircleAlphaOuter",
                    "ChallengeLoadScreen.wnd:CircleAlphaInner",
                    "ChallengeLoadScreen.wnd:VersusBackdrop",
                    "ChallengeLoadScreen.wnd:OverlayVs",
                ]
                .into_iter(),
            )
        {
            named_test_window(wm, name, effects);
        }
    }

    fn setup_current_challenge_for_tests(
        movie_label: &str,
        client: &mut crate::core::game_client::GameClient,
    ) {
        Language::clear_localized_strings();
        Language::register_localized_string("CHALLENGE:PlayerName", "General Player");
        Language::register_localized_string("CHALLENGE:PlayerRank", "General");
        Language::register_localized_string("CHALLENGE:PlayerStrategy", "Air superiority");
        Language::register_localized_string("CHALLENGE:OpponentName", "General Opponent");
        Language::register_localized_string("CHALLENGE:OpponentRank", "Prince");
        Language::register_localized_string("CHALLENGE:OpponentStrategy", "Ambush");

        let mut generals = ChallengeGenerals::new();
        let positions = generals.challenge_generals_mut();
        positions[0] = GeneralPersona::new();
        positions[0].set_campaign("challengecampaign".to_string());
        positions[0].set_bio_name("CHALLENGE:PlayerName".to_string());
        positions[0].set_bio_rank("CHALLENGE:PlayerRank".to_string());
        positions[0].set_bio_strategy("CHALLENGE:PlayerStrategy".to_string());
        positions[0].set_bio_portrait_large(Some("PlayerPortrait".to_string()));
        positions[0].set_portrait_movie_left_name("PlayerMovieLeft".to_string());
        positions[0].set_portrait_movie_right_name("PlayerMovieRight".to_string());
        positions[0].set_name_sound("PlayerNameSound".to_string());

        positions[1] = GeneralPersona::new();
        positions[1].set_bio_name("CHALLENGE:OpponentName".to_string());
        positions[1].set_bio_rank("CHALLENGE:OpponentRank".to_string());
        positions[1].set_bio_strategy("CHALLENGE:OpponentStrategy".to_string());
        positions[1].set_bio_portrait_large(Some("OpponentPortrait".to_string()));
        positions[1].set_portrait_movie_left_name("OpponentMovieLeft".to_string());
        positions[1].set_portrait_movie_right_name("OpponentMovieRight".to_string());
        positions[1].set_name_sound("OpponentNameSound".to_string());
        client.admit_challenge_generals(generals);

        let manager = client.campaign_manager_mut();
        {
            let campaign = manager.new_campaign("challengecampaign".to_string());
            campaign.first_mission = "mission1".to_string();
            campaign.is_challenge_campaign = true;
            let mission = campaign.new_mission("mission1".to_string());
            mission.general_name = "CHALLENGE:OpponentName".to_string();
            mission.movie_label = movie_label.to_string();
        }
        manager.set_campaign_and_mission("challengecampaign", "mission1");
    }

    fn cache_challenge_test_personas(runtime: &mut LoadScreenOperation<'_>) {
        with_challenge_load_screen_state(
            |state| {
                *state = ChallengeLoadScreenState {
                    player: Some(ChallengePersonaText {
                        big_name: "General Player".to_string(),
                        name: "General Player".to_string(),
                        rank: "General".to_string(),
                        strategy: "Air superiority".to_string(),
                        portrait_large: Some("PlayerPortrait".to_string()),
                        portrait_movie_left: "PlayerMovieLeft".to_string(),
                        portrait_movie_right: "PlayerMovieRight".to_string(),
                        name_sound: "PlayerNameSound".to_string(),
                        taunt_sounds: [
                            "PlayerTaunt1".to_string(),
                            "PlayerTaunt2".to_string(),
                            "PlayerTaunt3".to_string(),
                        ],
                    }),
                    opponent: Some(ChallengePersonaText {
                        big_name: "General Opponent".to_string(),
                        name: "General Opponent".to_string(),
                        rank: "Prince".to_string(),
                        strategy: "Ambush".to_string(),
                        portrait_large: Some("OpponentPortrait".to_string()),
                        portrait_movie_left: "OpponentMovieLeft".to_string(),
                        portrait_movie_right: "OpponentMovieRight".to_string(),
                        name_sound: "OpponentNameSound".to_string(),
                        taunt_sounds: [
                            "OpponentTaunt1".to_string(),
                            "OpponentTaunt2".to_string(),
                            "OpponentTaunt3".to_string(),
                        ],
                    }),
                    ..ChallengeLoadScreenState::default()
                };
            },
            runtime,
        );
    }
}

#[cfg(test)]
mod runtime_owner_tests {
    include!("tests/owner.rs");

    use super::fixture_owner::{AudioTrace, LoadScreenFixture, accepted_handle};
    use super::*;
}
