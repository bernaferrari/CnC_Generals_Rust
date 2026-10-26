//! Receiving menu/definition ownership, not complete Challenge campaign acceptance.
use game_client_rust::core::game_client::{ClientInputAuthority, GameClient};
use game_client_rust::gui::callbacks::challenge_menu::*;
use game_client_rust::gui::challenge_generals::ChallengeGenerals;
use game_client_rust::gui::menu_audio::SilentMenuAudio;
use game_client_rust::gui::{GameWindow, WindowLayout, WindowMessage, WindowWidget};
use game_engine::common::name_key_generator::NameKeyGenerator;
use std::{cell::RefCell, rc::Rc};
type Window = Rc<RefCell<GameWindow>>;
fn id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(&format!("ChallengeMenu.wnd:{name}")) as i32
}
fn client(label: &str) -> GameClient {
    let mut client =
        GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap();
    client
        .set_in_game_ui_subsystem(game_client_rust::core::subsystems::InGameUISubsystem::default());
    let mut definitions = ChallengeGenerals::new();
    for index in 0..2 {
        let persona = &mut definitions.challenge_generals_mut()[index];
        persona.set_starts_enabled(true);
        persona.set_bio_name(format!("{label}Name{index}"));
        persona.set_preview_sound(format!("{label}Preview{index}"));
    }
    client.admit_challenge_generals(definitions);
    client
}
struct Scene {
    layout: WindowLayout,
    parent: Window,
    generals: [Window; 2],
    play: Window,
}
fn scene(client: &mut GameClient) -> Scene {
    client.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            let parent = manager
                .create_window_with_id(None, 0, 0, 400, 400, id("ParentChallengeMenu"), effects)
                .unwrap();
            parent
                .borrow_mut()
                .set_system_callback_with_message_sink(challenge_menu_system);
            let mut layout = WindowLayout::new("Menus/ChallengeMenu.wnd".into());
            layout.add_window(parent.clone());
            let mut generals = Vec::new();
            let mut play = None;
            for name in [
                "ButtonPlay",
                "ButtonBack",
                "GadgetParent",
                "GeneralsBioParent",
                "BioPortrait",
                "BioNameEntry",
                "BioDOBEntry",
                "BioBirthplaceEntry",
                "BioStrategyEntry",
                "GeneralPosition0",
                "GeneralPosition1",
            ] {
                let window = manager
                    .create_window_with_id(Some(&parent), 0, 0, 40, 40, id(name), effects)
                    .unwrap();
                window
                    .borrow_mut()
                    .set_name(&format!("ChallengeMenu.wnd:{name}"));
                window.borrow_mut().set_owner(Some(&parent));
                if name.starts_with("GeneralPosition") {
                    window.borrow_mut().set_widget(WindowWidget::CheckBox(
                        game_client_rust::gui::gadgets::CheckBox::new(id(name) as u32, 0, 0, 40),
                    ));
                    generals.push(window.clone());
                }
                if name == "ButtonPlay" {
                    play = Some(window.clone());
                }
                layout.add_window(window);
            }
            {
                let mut generals = generals.into_iter();
                Scene {
                    layout,
                    parent,
                    generals: [generals.next().unwrap(), generals.next().unwrap()],
                    play: play.unwrap(),
                }
            }
        })
    })
}
fn init(client: &mut GameClient, scene: &Scene) {
    let mut silent = SilentMenuAudio::new();
    client.with_window_event_effects_and_menu_audio(None, &mut silent, |effects| {
        challenge_menu_init(&scene.layout, None, effects);
    });
}
fn select(client: &mut GameClient, scene: &Scene, index: usize) {
    let mut silent = SilentMenuAudio::new();
    client.with_window_event_effects_and_menu_audio(None, &mut silent, |effects| {
        // This is a real parent callback with its RefMut held. Automatic
        // checkbox echoes must reenter that exact mutable recipient directly.
        scene.parent.borrow_mut().send_system_message(
            WindowMessage::GadgetSelected,
            id(&format!("GeneralPosition{index}")) as usize,
            0,
            effects,
        );
    });
}
fn checked(window: &Window) -> bool {
    match window.borrow().widget() {
        Some(WindowWidget::CheckBox(checkbox)) => checkbox.is_checked(),
        _ => panic!("authored general checkbox"),
    }
}
#[test]
fn equal_id_challenge_menus_keep_selection_widgets_and_catalogs_independent() {
    let mut a = client("Owner");
    let mut b = client("Peer");
    let first = scene(&mut a);
    let second = scene(&mut b);
    assert_eq!(
        first.parent.borrow().get_id(),
        second.parent.borrow().get_id()
    );
    init(&mut a, &first);
    init(&mut b, &second);
    select(&mut a, &first, 0);
    select(&mut b, &second, 1);
    assert!(checked(&first.generals[0]));
    assert!(!checked(&first.generals[1]));
    assert!(!checked(&second.generals[0]));
    assert!(checked(&second.generals[1]));
    select(&mut a, &first, 1);
    assert!(!checked(&first.generals[0]));
    assert!(checked(&first.generals[1]));
    select(&mut a, &first, 1);
    assert!(
        checked(&first.generals[1]),
        "reselecting the current general retains its check"
    );
    assert_eq!(
        a.challenge_generals().challenge_generals()[0].bio_name(),
        "OwnerName0"
    );
    assert_eq!(
        b.challenge_generals().challenge_generals()[0].bio_name(),
        "PeerName0"
    );
    assert!(!first.play.borrow().is_hidden());
    // GameClient.cpp426-452 resets match presentation, not the Shell menu.
    // Keep original menu receipts until its authored shutdown, and prove that
    // resetting a peer cannot touch this client's same-ID selection/catalog.
    b.reset().unwrap();
    assert!(checked(&first.generals[1]));
    assert!(checked(&second.generals[1]));
    assert_eq!(
        a.challenge_generals().challenge_generals()[1].bio_name(),
        "OwnerName1"
    );
    let mut silent = SilentMenuAudio::new();
    b.with_window_event_effects_and_menu_audio(None, &mut silent, |effects| {
        challenge_menu_shutdown(&second.layout, Some(&true), effects);
    });
    assert!(second.parent.borrow().is_hidden());
    assert!(!first.parent.borrow().is_hidden());
    drop(second);
    drop(b);
    assert!(checked(&first.generals[1]));
    assert_eq!(
        a.challenge_generals().challenge_generals()[1].bio_name(),
        "OwnerName1"
    );
}
#[test]
fn challenge_runtime_difficulty_and_template_belong_to_campaign_not_persona_definitions() {
    use game_client_rust::gui::campaign_manager::GameDifficulty;
    use game_client_rust::gui::challenge_generals::{
        simulate_challenge_generals_set_difficulty, simulate_challenge_generals_set_template_num,
    };
    let mut a = client("Owner");
    let mut b = client("Peer");
    assert!(simulate_challenge_generals_set_difficulty(
        a.campaign_manager_mut(),
        2
    ));
    assert!(simulate_challenge_generals_set_template_num(
        a.campaign_manager_mut(),
        7
    ));
    assert!(simulate_challenge_generals_set_difficulty(
        b.campaign_manager_mut(),
        0
    ));
    assert!(simulate_challenge_generals_set_template_num(
        b.campaign_manager_mut(),
        2
    ));
    assert_eq!(
        a.campaign_manager().get_game_difficulty(),
        GameDifficulty::Hard
    );
    assert_eq!(
        b.campaign_manager().get_game_difficulty(),
        GameDifficulty::Easy
    );
    assert_eq!(
        a.campaign_manager()
            .get_xfer_challenge_generals_player_template_num(),
        7
    );
    assert_eq!(
        b.campaign_manager()
            .get_xfer_challenge_generals_player_template_num(),
        2
    );
    drop(b);
    assert_eq!(
        a.campaign_manager()
            .get_xfer_challenge_generals_player_template_num(),
        7
    );
    assert_eq!(
        a.challenge_generals().challenge_generals()[0].bio_name(),
        "OwnerName0"
    );
}
