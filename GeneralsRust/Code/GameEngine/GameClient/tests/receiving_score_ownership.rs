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
            ok.borrow_mut().hide(false, manager);
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
            if let Some(end_frame) = end_frame {
                parent
                    .borrow_mut()
                    .set_user_data(ScoreScreenAcademySnapshot {
                        end_frame,
                        show_advice: false,
                        advice: Default::default(),
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
