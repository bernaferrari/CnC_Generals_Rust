//! Campaign-popup ownership and callback order, not full campaign-playability acceptance.
use game_client_rust::core::game_client::{ClientInputAuthority, GameClient};
use game_client_rust::gui::callbacks::difficulty_select::*;
use game_client_rust::gui::campaign_launch_host_bridge::MenuWorldFacts;
use game_client_rust::gui::{GameWindow, WindowLayout, WindowMessage};
use game_engine::common::name_key_generator::NameKeyGenerator;
use std::{cell::RefCell, rc::Rc};
type Window = Rc<RefCell<GameWindow>>;
fn facts() -> MenuWorldFacts {
    MenuWorldFacts {
        script_engine_available: false,
        chat: None,
        game_mode: gamelogic::system::game_logic::GAME_NONE,
        client_frame: 0,
        logic_frame: 0,
        in_game: false,
        in_shell_game: false,
    }
}
fn id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(&format!("DifficultySelect.wnd:{name}")) as i32
}
fn client() -> GameClient {
    GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap()
}
struct Scene {
    layout: Rc<RefCell<WindowLayout>>,
    parent: Window,
}
fn scene(c: &mut GameClient, events: Rc<RefCell<Vec<&'static str>>>) -> Scene {
    c.with_window_event_effects(Some(facts()), |effects| {
        effects.with_manager(|manager, effects| {
            let parent = manager
                .create_window_with_id(None, 0, 0, 200, 200, id("DifficultySelectParent"), effects)
                .unwrap();
            parent
                .borrow_mut()
                .set_name("DifficultySelect.wnd:DifficultySelectParent");
            let e = events.clone();
            parent.borrow_mut().set_system_callback_with_message_sink(
                move |window, msg, a, b, effects| {
                    if msg == WindowMessage::Destroy {
                        e.borrow_mut().push("parent");
                    }
                    if msg == WindowMessage::InputFocus {
                        e.borrow_mut().push("focus");
                    }
                    difficulty_select_system(window, msg, a, b, effects)
                },
            );
            let layout = Rc::new(RefCell::new(WindowLayout::new(
                "Menus/DifficultySelect.wnd".into(),
            )));
            layout.borrow_mut().add_window(parent.clone());
            parent.borrow_mut().set_layout(Some(&layout));
            for name in [
                "ButtonOk",
                "ButtonCancel",
                "RadioButtonEasy",
                "RadioButtonMedium",
                "RadioButtonHard",
            ] {
                let w = manager
                    .create_window_with_id(Some(&parent), 0, 0, 20, 20, id(name), effects)
                    .unwrap();
                w.borrow_mut()
                    .set_name(&format!("DifficultySelect.wnd:{name}"));
                w.borrow_mut().set_layout(Some(&layout));
                let e = events.clone();
                w.borrow_mut()
                    .set_system_callback_with_message_sink(move |_, msg, _, _, _| {
                        if msg == WindowMessage::Destroy {
                            e.borrow_mut().push("child");
                        }
                        game_client_rust::gui::WindowMsgHandled::Handled
                    });
                layout.borrow_mut().add_window(w);
            }
            Scene { layout, parent }
        })
    })
}
fn init(c: &mut GameClient, s: &Scene) {
    c.with_window_event_effects(Some(facts()), |effects| {
        difficulty_select_init(&s.layout.borrow(), None, effects)
    });
}
fn select(c: &mut GameClient, s: &Scene, name: &str) {
    c.with_window_event_effects(Some(facts()), |effects| {
        let _ = s.parent.borrow_mut().send_system_message(
            WindowMessage::GadgetSelected,
            id(name) as usize,
            0,
            effects,
        );
    });
}
fn observed_difficulty(c: &mut GameClient) -> u8 {
    c.with_window_event_effects(Some(facts()), |effects| {
        assert!(simulate_difficulty_select_ok_button_gadget_selected(
            effects
        ));
        residual_difficulty_select_level(effects)
    })
}
#[test]
fn same_ids_select_independently_without_global_state() {
    let mut a = client();
    let mut b = client();
    let sa = scene(&mut a, Default::default());
    let sb = scene(&mut b, Default::default());
    init(&mut a, &sa);
    init(&mut b, &sb);
    assert_eq!(sa.parent.borrow().get_id(), sb.parent.borrow().get_id());
    select(&mut a, &sa, "RadioButtonHard");
    select(&mut b, &sb, "RadioButtonEasy");
    assert_eq!(observed_difficulty(&mut a), 2);
    assert_eq!(observed_difficulty(&mut b), 0);
    b.reset().unwrap();
    drop(b);
    assert_eq!(observed_difficulty(&mut a), 2);
}
#[test]
fn cancel_retires_original_layout_after_source_loan_and_before_return() {
    let mut a = client();
    let mut b = client();
    let events = Rc::new(RefCell::new(Vec::new()));
    let sa = scene(&mut a, events.clone());
    let sb = scene(&mut b, Default::default());
    init(&mut a, &sa);
    init(&mut b, &sb);
    assert!(
        events.borrow().is_empty(),
        "C++ DifficultySelect init does not request keyboard focus"
    );
    select(&mut a, &sa, "ButtonCancel");
    assert_eq!(events.borrow().first(), Some(&"parent"));
    assert_eq!(events.borrow().iter().filter(|e| **e == "child").count(), 5);
    a.window_manager_owner().with_manager_ref(|manager| {
        assert!(
            manager
                .get_window_by_id(id("DifficultySelectParent"))
                .is_none()
        )
    });
    b.window_manager_owner().with_manager_ref(|manager| {
        assert!(
            manager
                .get_window_by_id(id("DifficultySelectParent"))
                .is_some()
        )
    });
    select(&mut b, &sb, "RadioButtonHard");
    assert_eq!(observed_difficulty(&mut b), 2);
}
