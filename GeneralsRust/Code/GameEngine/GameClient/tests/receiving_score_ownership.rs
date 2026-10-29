//! UI-owner contracts only: this is not a completed-gameplay parity claim.
use game_client_rust::core::game_client::{ClientInputAuthority, GameClient};
use game_client_rust::core::subsystems::InGameUISubsystem;
use game_client_rust::gui::callbacks::score_screen::*;
use game_client_rust::gui::menu_flags::*;
use game_client_rust::gui::{GameWindow, WindowLayout};
use game_engine::common::name_key_generator::NameKeyGenerator;
use std::{cell::RefCell, rc::Rc};

type Window = Rc<RefCell<GameWindow>>;
struct ScoreScene {
    layout: WindowLayout,
    parent: Window,
    ok: Window,
    save: Window,
}
fn client() -> GameClient {
    let mut client =
        GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap();
    client.set_in_game_ui_subsystem(InGameUISubsystem::default());
    client
}
fn id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(name) as i32
}
fn scene(client: &mut GameClient, end_frame: Option<u32>) -> ScoreScene {
    client.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            let parent = manager
                .create_window_with_id(
                    None,
                    0,
                    0,
                    200,
                    200,
                    id("ScoreScreen.wnd:ParentScoreScreen"),
                    effects,
                )
                .unwrap();
            parent
                .borrow_mut()
                .set_name("ScoreScreen.wnd:ParentScoreScreen");
            parent
                .borrow_mut()
                .set_system_callback_with_message_sink(score_screen_system);
            parent
                .borrow_mut()
                .set_input_callback_with_message_sink(score_screen_input);
            let ok = manager
                .create_window_with_id(
                    Some(&parent),
                    0,
                    0,
                    20,
                    20,
                    id("ScoreScreen.wnd:ButtonOk"),
                    effects,
                )
                .unwrap();
            ok.borrow_mut().set_name("ScoreScreen.wnd:ButtonOk");
            let _ = ok.borrow_mut().hide(false, manager);
            let save = manager
                .create_window_with_id(
                    Some(&parent),
                    20,
                    0,
                    20,
                    20,
                    id("ScoreScreen.wnd:ButtonSaveReplay"),
                    effects,
                )
                .unwrap();
            save.borrow_mut()
                .set_name("ScoreScreen.wnd:ButtonSaveReplay");
            for index in 0..8 {
                for name in ["StaticTextPlayer", "StaticTextResources"] {
                    let name = format!("ScoreScreen.wnd:{name}{index}");
                    let child = manager
                        .create_window_with_id(Some(&parent), 0, 0, 20, 20, id(&name), effects)
                        .unwrap();
                    child.borrow_mut().set_name(&name);
                }
            }
            if let Some(end_frame) = end_frame {
                parent
                    .borrow_mut()
                    .set_user_data(ScoreScreenAcademySnapshot {
                        end_frame,
                        show_advice: false,
                        advice: Default::default(),
                        player_roster: None,
                        is_single_player: None,
                        is_skirmish: None,
                        session_difficulty: None,
                        skirmish_outcome: None,
                    });
            }
            let mut layout = WindowLayout::new("Menus/ScoreScreen.wnd".into());
            layout.add_window(parent.clone());
            ScoreScene {
                layout,
                parent,
                ok,
                save,
            }
        })
    })
}

#[test]
fn native_score_rows_use_the_receiving_roster_and_relationships() {
    use gamelogic::common::Relationship;

    fn player(
        id: i32,
        player_name: &str,
        display_name: &str,
        side: &str,
        local: bool,
        relationship: Relationship,
        money: i32,
    ) -> ScoreScreenPlayerSnapshot {
        let mut score_keeper = game_engine::common::rts::score_keeper::ScoreKeeper::new();
        score_keeper.add_money_earned(money);
        ScoreScreenPlayerSnapshot {
            id,
            player_name: player_name.into(),
            display_name: display_name.into(),
            side: side.into(),
            base_side: side.into(),
            color_argb: 0xff11_2233,
            side_icon: format!("{side}Icon"),
            score_screen_image: format!("{side}Score"),
            is_local: local,
            is_observer: false,
            is_human: true,
            is_neutral: false,
            is_active: true,
            has_objects: true,
            list_in_score_screen: true,
            relationship_to_local: relationship,
            rank_points: 0,
            score_keeper,
        }
    }
    fn install(
        scene: &ScoreScene,
        local_name: &str,
        peer_name: &str,
        peer_side: &str,
        relationship: Relationship,
        peer_money: i32,
        is_single_player: bool,
    ) {
        let roster = vec![
            player(
                0,
                "player0",
                local_name,
                "USA",
                true,
                Relationship::Allies,
                101,
            ),
            player(
                1,
                "player1",
                peer_name,
                peer_side,
                false,
                relationship,
                peer_money,
            ),
        ];
        scene
            .parent
            .borrow_mut()
            .set_user_data(ScoreScreenAcademySnapshot {
                end_frame: 100,
                show_advice: false,
                advice: Default::default(),
                player_roster: Some(roster),
                is_single_player: Some(is_single_player),
                is_skirmish: Some(!is_single_player),
                session_difficulty: Some(1),
                skirmish_outcome: None,
            });
    }
    fn text(scene: &ScoreScene, name: &str) -> String {
        scene
            .parent
            .borrow()
            .find_child_by_id(id(name))
            .unwrap()
            .borrow()
            .get_text()
            .to_owned()
    }

    let mut first_client = client();
    let mut second_client = client();
    let first = scene(&mut first_client, None);
    let second = scene(&mut second_client, None);
    install(
        &first,
        "FirstMatchLocal",
        "FirstMatchPeer",
        "China",
        Relationship::Allies,
        31,
        true,
    );
    install(
        &second,
        "SecondMatchLocal",
        "SecondMatchPeer",
        "GLA",
        Relationship::Enemies,
        79,
        false,
    );
    first_client.with_window_event_effects(None, |effects| {
        score_screen_init(&first.layout, None, effects)
    });
    second_client.with_window_event_effects(None, |effects| {
        score_screen_init(&second.layout, None, effects)
    });

    assert_eq!(
        text(&first, "ScoreScreen.wnd:StaticTextPlayer0"),
        game_client_rust::game_text::GameText::fetch("GUI:Player")
    );
    assert!(first.save.borrow().is_hidden());
    assert_eq!(
        text(&second, "ScoreScreen.wnd:StaticTextPlayer0"),
        "SecondMatchLocal"
    );
    assert_eq!(
        text(&first, "ScoreScreen.wnd:StaticTextPlayer1"),
        game_client_rust::game_text::GameText::fetch("GUI:ChinaAllies")
    );
    assert_eq!(
        text(&second, "ScoreScreen.wnd:StaticTextPlayer1"),
        "SecondMatchPeer"
    );
    assert_eq!(text(&first, "ScoreScreen.wnd:StaticTextResources1"), "31");
    assert_eq!(text(&second, "ScoreScreen.wnd:StaticTextResources1"), "79");
}

#[test]
fn score_widgets_flags_and_residuals_stay_with_the_receiving_client() {
    let mut a = client();
    let mut b = client();
    a.with_window_event_effects(None, |effects| set_dont_show_main_menu(effects, true));
    assert!(a.with_window_event_effects(None, |effects| get_dont_show_main_menu(effects)));
    assert!(!b.with_window_event_effects(None, |effects| get_dont_show_main_menu(effects)));
    let first = scene(&mut a, Some(18000));
    let second = scene(&mut b, Some(700));
    assert_eq!(
        first.parent.borrow().get_id(),
        second.parent.borrow().get_id()
    );
    assert_eq!(first.ok.borrow().get_id(), second.ok.borrow().get_id());
    assert_eq!(first.save.borrow().get_id(), second.save.borrow().get_id());
    a.with_window_event_effects(None, |effects| {
        score_screen_init(&first.layout, None, effects)
    });
    b.with_window_event_effects(None, |effects| {
        score_screen_init(&second.layout, None, effects)
    });
    assert!(first.ok.borrow().is_enabled());
    assert!(second.ok.borrow().is_enabled());
    // Real selected-parent loan remains held. Controls must use cached child
    // receipts instead of reopening the selected parent's RefCell.
    let selected = second.parent.borrow_mut();
    b.with_window_event_effects(None, |effects| score_screen_enable_controls(false, effects));
    drop(selected);
    assert!(first.ok.borrow().is_enabled());
    assert!(!second.ok.borrow().is_enabled());
    a.with_window_event_effects(None, |effects| {
        simulate_score_screen_set_finish_campaign(true, effects);
        simulate_score_screen_ok_button_gadget_selected(effects);
    });
    b.with_window_event_effects(None, |effects| {
        simulate_score_screen_set_finish_campaign(false, effects);
        simulate_score_screen_emote_button_gadget_selected(effects);
        set_dont_show_main_menu(effects, false);
    });
    assert!(
        a.with_window_event_effects(None, |effects| residual_score_screen_is_finish_campaign(
            effects
        ))
    );
    assert!(
        !b.with_window_event_effects(None, |effects| residual_score_screen_is_finish_campaign(
            effects
        ))
    );
    assert_eq!(
        a.with_window_event_effects(None, |effects| residual_score_screen_last_action(effects)),
        ResidualScoreScreenAction::Ok
    );
    assert_eq!(
        b.with_window_event_effects(None, |effects| residual_score_screen_last_action(effects)),
        ResidualScoreScreenAction::Emote
    );
    drop(b);
    assert!(a.with_window_event_effects(None, |effects| get_dont_show_main_menu(effects)));
    assert_eq!(
        a.with_window_event_effects(None, |effects| residual_score_screen_last_action(effects)),
        ResidualScoreScreenAction::Ok
    );
    assert!(first.ok.borrow().is_enabled());
}

#[test]
fn native_score_admission_distinguishes_missing_frame_from_bound_zero() {
    let mut a = client();
    let mut b = client();
    let missing = scene(&mut a, None);
    let zero = scene(&mut b, Some(0));
    assert!(validate_completed_score_snapshot(&missing.layout).is_err());
    assert!(validate_completed_score_snapshot(&zero.layout).is_ok());
    assert!(validate_completed_score_snapshot(&missing.layout).is_err());
}

struct ReplayScene {
    layout: WindowLayout,
    parent: Window,
    list: Window,
    entry: Window,
}
fn replay_scene(client: &mut GameClient) -> ReplayScene {
    client.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            let parent = manager
                .create_window_with_id(
                    None,
                    0,
                    0,
                    200,
                    200,
                    id("PopupReplay.wnd:PopupReplayMenu"),
                    effects,
                )
                .unwrap();
            let list = manager
                .create_window_with_id(
                    Some(&parent),
                    0,
                    0,
                    100,
                    60,
                    id("PopupReplay.wnd:ListboxGames"),
                    effects,
                )
                .unwrap();
            list.borrow_mut()
                .set_widget(game_client_rust::gui::WindowWidget::ListBox(
                    game_client_rust::gui::gadgets::ListBox::new(
                        id("PopupReplay.wnd:ListboxGames") as u32,
                        0,
                        0,
                        100,
                        60,
                    ),
                ));
            let entry = manager
                .create_window_with_id(
                    Some(&parent),
                    0,
                    60,
                    100,
                    20,
                    id("PopupReplay.wnd:TextEntryReplayName"),
                    effects,
                )
                .unwrap();
            entry
                .borrow_mut()
                .set_widget(game_client_rust::gui::WindowWidget::TextEntry(
                    game_client_rust::gui::gadgets::TextEntry::new(
                        id("PopupReplay.wnd:TextEntryReplayName") as u32,
                        0,
                        60,
                        100,
                        20,
                    ),
                ));
            let mut layout = WindowLayout::new("Menus/PopupReplay.wnd".into());
            layout.add_window(parent.clone());
            game_client_rust::gui::callbacks::popup_replay::popup_replay_init(
                &layout, None, manager, effects,
            );
            ReplayScene {
                layout,
                parent,
                list,
                entry,
            }
        })
    })
}
fn append_replay(scene: &ReplayScene, name: &str) -> usize {
    let mut window = scene.list.borrow_mut();
    let list = window.list_box_mut().unwrap();
    let row = list.items().len();
    list.add_item(name);
    row
}
fn select_replay(client: &mut GameClient, scene: &ReplayScene, row: usize) {
    client.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            let mut parent = scene.parent.borrow_mut();
            assert_eq!(
                game_client_rust::gui::callbacks::popup_replay::popup_replay_system(
                    &mut parent,
                    game_client_rust::gui::WindowMessage::User(game_client_rust::gui::GLM_SELECTED),
                    id("PopupReplay.wnd:ListboxGames") as usize,
                    row,
                    manager,
                    effects
                ),
                game_client_rust::gui::WindowMsgHandled::Handled
            );
        })
    });
}
fn replay_text(scene: &ReplayScene) -> String {
    let window = scene.entry.borrow();
    match window.widget() {
        Some(game_client_rust::gui::WindowWidget::TextEntry(entry)) => entry.text().to_owned(),
        _ => panic!("receiving replay name widget must remain installed"),
    }
}

fn score_row(
    id: i32,
    player_name: &str,
    display_name: &str,
    side: &str,
    local: bool,
    observer: bool,
    human: bool,
    relationship: gamelogic::common::Relationship,
    money: i32,
) -> ScoreScreenPlayerSnapshot {
    let mut score_keeper = game_engine::common::rts::score_keeper::ScoreKeeper::new();
    score_keeper.add_money_earned(money);
    ScoreScreenPlayerSnapshot {
        id,
        player_name: player_name.into(),
        display_name: display_name.into(),
        side: side.into(),
        base_side: side.into(),
        color_argb: 0xff11_2233,
        side_icon: format!("{side}Icon"),
        score_screen_image: format!("{side}Score"),
        is_local: local,
        is_observer: observer,
        is_human: human,
        is_neutral: false,
        is_active: true,
        has_objects: true,
        list_in_score_screen: true,
        relationship_to_local: relationship,
        rank_points: 0,
        score_keeper,
    }
}

fn install_roster(scene: &ScoreScene, roster: Vec<ScoreScreenPlayerSnapshot>, single: bool) {
    scene
        .parent
        .borrow_mut()
        .set_user_data(ScoreScreenAcademySnapshot {
            end_frame: 100,
            show_advice: false,
            advice: Default::default(),
            player_roster: Some(roster),
            is_single_player: Some(single),
            is_skirmish: Some(!single),
            session_difficulty: Some(1),
            skirmish_outcome: None,
        });
}

fn score_text(scene: &ScoreScene, name: &str) -> String {
    scene
        .parent
        .borrow()
        .find_child_by_id(id(name))
        .unwrap()
        .borrow()
        .get_text()
        .to_owned()
}

#[test]
fn multiplayer_score_map_replaces_adjusted_collision_and_hides_from_input_count() {
    use gamelogic::common::Relationship;
    let mut client = client();
    let scene = scene(&mut client, None);
    install_roster(
        &scene,
        vec![
            score_row(
                0,
                "player0",
                "Original102",
                "USA",
                true,
                false,
                true,
                Relationship::Allies,
                102,
            ),
            score_row(
                1,
                "player1",
                "First101",
                "China",
                false,
                false,
                true,
                Relationship::Enemies,
                101,
            ),
            score_row(
                2,
                "player2",
                "AdjustedCollision",
                "GLA",
                false,
                false,
                true,
                Relationship::Enemies,
                101,
            ),
        ],
        false,
    );
    client.with_window_event_effects(None, |effects| {
        score_screen_init(&scene.layout, None, effects)
    });

    // C++ first changes the duplicate 101 to 102, then inserts into the
    // score-keyed map, replacing the earlier 102 row. hideWindows still gets
    // the three discovered slots, not the two surviving map entries.
    assert_eq!(
        score_text(&scene, "ScoreScreen.wnd:StaticTextPlayer0"),
        "AdjustedCollision"
    );
    assert_eq!(
        score_text(&scene, "ScoreScreen.wnd:StaticTextPlayer1"),
        "First101"
    );
    assert!(
        !scene
            .parent
            .borrow()
            .find_child_by_id(id("ScoreScreen.wnd:StaticTextPlayer2"))
            .unwrap()
            .borrow()
            .is_hidden()
    );
}

#[test]
fn single_player_rows_order_all_faction_friends_before_enemies() {
    use gamelogic::common::Relationship;
    let mut client = client();
    let scene = scene(&mut client, None);
    let mut roster = vec![score_row(
        0,
        "player0",
        "Local",
        "USA",
        true,
        false,
        true,
        Relationship::Allies,
        1,
    )];
    for (index, side, relationship) in [
        (1, "USA", Relationship::Allies),
        (2, "China", Relationship::Allies),
        (3, "GLA", Relationship::Allies),
        (4, "USA", Relationship::Enemies),
        (5, "China", Relationship::Enemies),
        (6, "GLA", Relationship::Enemies),
    ] {
        roster.push(score_row(
            index,
            &format!("player{index}"),
            &format!("{side}{index}"),
            side,
            false,
            false,
            true,
            relationship,
            index * 10,
        ));
    }
    install_roster(&scene, roster, true);
    client.with_window_event_effects(None, |effects| {
        score_screen_init(&scene.layout, None, effects)
    });

    let expected = [
        "GUI:USAAllies",
        "GUI:ChinaAllies",
        "GUI:GLAAllies",
        "GUI:USAEnemies",
        "GUI:ChinaEnemies",
        "GUI:GLAEnemies",
    ];
    for (index, label) in expected.into_iter().enumerate() {
        assert_eq!(
            score_text(
                &scene,
                &format!("ScoreScreen.wnd:StaticTextPlayer{}", index + 1)
            ),
            game_client_rust::game_text::GameText::fetch(label),
        );
    }
}

#[test]
fn observer_single_player_score_uses_first_human_for_exclusion() {
    use gamelogic::common::Relationship;
    let mut client = client();
    let scene = scene(&mut client, None);
    install_roster(
        &scene,
        vec![
            score_row(
                0,
                "player0",
                "Observer",
                "Neutral",
                true,
                true,
                false,
                Relationship::Neutral,
                1,
            ),
            score_row(
                1,
                "player1",
                "FirstHuman",
                "USA",
                false,
                false,
                true,
                Relationship::Allies,
                10,
            ),
            score_row(
                2,
                "player2",
                "USAFriend",
                "USA",
                false,
                false,
                true,
                Relationship::Allies,
                15,
            ),
        ],
        true,
    );
    client.with_window_event_effects(None, |effects| {
        score_screen_init(&scene.layout, None, effects)
    });

    assert_eq!(
        score_text(&scene, "ScoreScreen.wnd:StaticTextPlayer0"),
        game_client_rust::game_text::GameText::fetch("GUI:Player")
    );
    assert_eq!(
        score_text(&scene, "ScoreScreen.wnd:StaticTextPlayer1"),
        game_client_rust::game_text::GameText::fetch("GUI:USAAllies")
    );
    assert_eq!(
        score_text(&scene, "ScoreScreen.wnd:StaticTextResources1"),
        "15"
    );
}

#[test]
fn replay_popup_selection_and_shutdown_use_only_the_receiving_windows() {
    let mut a = client();
    let mut b = client();
    let first = replay_scene(&mut a);
    let second = replay_scene(&mut b);
    assert_eq!(first.list.borrow().get_id(), second.list.borrow().get_id());
    let first_row = append_replay(&first, "OwnerReplay");
    let second_row = append_replay(&second, "PeerReplay");
    select_replay(&mut a, &first, first_row);
    assert_eq!(replay_text(&first), "OwnerReplay");
    assert_eq!(replay_text(&second), "");
    select_replay(&mut b, &second, second_row);
    assert_eq!(replay_text(&first), "OwnerReplay");
    assert_eq!(replay_text(&second), "PeerReplay");
    b.with_window_event_effects(None, |effects| {
        game_client_rust::gui::callbacks::popup_replay::popup_replay_shutdown(
            &second.layout,
            None,
            effects,
        )
    });
    first
        .entry
        .borrow_mut()
        .text_entry_mut()
        .unwrap()
        .set_text("changed");
    select_replay(&mut a, &first, first_row);
    assert_eq!(replay_text(&first), "OwnerReplay");
    assert_eq!(replay_text(&second), "PeerReplay");
}
