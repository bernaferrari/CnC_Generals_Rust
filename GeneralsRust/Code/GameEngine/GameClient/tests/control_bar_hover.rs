#![cfg(not(target_arch = "wasm32"))]

use game_client_rust::core::game_client::GameClient;
use game_client_rust::gui::control_bar::ControlBar;
use game_client_rust::gui::gadgets::{Gadget, GadgetState};
use game_client_rust::gui::game_window::{WindowMessage, WindowState, WindowWidget};
use game_engine::common::system::big_file_system::BigFileSystem;
use gamelogic::system::engine_stores::{new_world_services, with_world_services};
use std::{io::Read, path::PathBuf, rc::Rc};

#[test]
#[ignore = "requires the installed Zero Hour WindowZH.big archive"]
fn available_stock_command_keeps_hover_and_press_until_input_or_disable() {
    let root = PathBuf::from(std::env::var_os("GENERALS_ASSETS_DIR").expect("retail install"));
    let mut archive = BigFileSystem::new();
    archive
        .open_archive_file(root.join("Data/WindowZH.big"))
        .unwrap();
    let mut layout = Vec::new();
    archive
        .open_file("Window/ControlBar.wnd", 1)
        .unwrap()
        .read_to_end(&mut layout)
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("ControlBar.wnd");
    std::fs::write(&path, layout).unwrap();

    let world = new_world_services();
    with_world_services(&world, || {
        let mut client = GameClient::new().unwrap();
        client.with_window_event_effects(None, |effects| {
            effects.with_manager(|manager, effects| {
                manager.set_screen_size(800, 600);
                manager
                    .load_window(path.to_str().unwrap(), effects)
                    .unwrap();
                ControlBar::reveal_ingame_command_window_owned(manager);
                let command = manager
                    .find_window_by_name("ControlBar.wnd:ButtonCommand01")
                    .unwrap();
                let mut ancestor = Some(Rc::clone(&command));
                while let Some(window) = ancestor {
                    let mut window = window.borrow_mut();
                    window.hide(false, manager).unwrap();
                    ancestor = window.get_parent();
                }
                let (x, y) = command.borrow().get_screen_position();
                let (w, h) = command.borrow().get_size();
                let (x, y) = (x + w / 2, y + h / 2);
                let data = (((y as u32) << 16) | ((x as u32) & 0xFFFF)) as usize;
                let hit = manager.get_window_under_cursor(x, y, false).unwrap();
                assert!(
                    Rc::ptr_eq(&command, &hit),
                    "stock hit at {x},{y} selected {}",
                    hit.borrow().get_name()
                );
                let state = || {
                    let window = command.borrow();
                    let Some(WindowWidget::PushButton(button)) = window.widget() else {
                        panic!("stock command is a PushButton");
                    };
                    (button.state(), window.instance_data().state)
                };

                manager.process_mouse_event(WindowMessage::MousePos, x, y, data, effects);
                assert_eq!(state().0, GadgetState::Hovered);
                assert!(state().1.contains(WindowState::HILITED));
                for _ in 0..3 {
                    // ControlBarCommand.cpp evaluates Available every frame.
                    command.borrow_mut().enable(true).unwrap();
                    manager.update(effects);
                    manager.process_mouse_event(WindowMessage::MousePos, x, y, data, effects);
                    assert_eq!(state().0, GadgetState::Hovered);
                    assert!(state().1.contains(WindowState::HILITED));
                }

                manager.process_mouse_event(WindowMessage::LeftDown, x, y, data, effects);
                assert_eq!(state().0, GadgetState::Pressed);
                for _ in 0..3 {
                    command.borrow_mut().enable(true).unwrap();
                    manager.update(effects);
                    assert_eq!(state().0, GadgetState::Pressed);
                    assert!(state().1.contains(WindowState::PUSHED));
                }
                manager.process_mouse_event(WindowMessage::LeftUp, x, y, data, effects);
                assert_eq!(state().0, GadgetState::Hovered);
                manager.process_mouse_event(WindowMessage::MousePos, -1, -1, usize::MAX, effects);
                assert_eq!(state().0, GadgetState::Normal);
                assert!(!state().1.contains(WindowState::HILITED));
                command.borrow_mut().enable(false).unwrap();
                manager.update(effects);
                assert_eq!(state().0, GadgetState::Disabled);
                command.borrow_mut().enable(true).unwrap();
                manager.update(effects);
                assert_eq!(state().0, GadgetState::Normal);

                manager.process_mouse_event(WindowMessage::MousePos, x, y, data, effects);
                assert_eq!(state().0, GadgetState::Hovered);
            });
        });
    });
}
