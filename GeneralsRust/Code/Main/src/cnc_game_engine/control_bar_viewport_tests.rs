//! Synchronous native UI adapter on independent authored window trees.
use super::NativeScriptUi;
use crate::game_logic::ScriptUiEffects;
use game_client::gui::control_bar::ControlBar;
use game_client::gui::game_window::WindowEventEffects;
use game_client::gui::window_manager::WindowManager;

#[test]
fn native_weather_adapter_only_changes_its_borrowed_snow() {
    let mut snow = game_client::snow::SnowManager::new();
    let peer = game_client::snow::SnowManager::new();
    let mut bar = ControlBar::new();
    for visible in [false, true, false] {
        NativeScriptUi::new(
            &mut snow,
            &mut bar,
            game_client::gui::window_manager::WindowManagerOwner::new(),
        )
        .weather_visible(visible);
        assert_eq!(snow.is_visible(), visible);
        assert!(peer.is_visible());
    }
}

#[test]
fn create_window_requires_effects_and_stays_local_to_manager() {
    WindowEventEffects::with_standalone(|effects| {
        let mut receiving = WindowManager::new();
        let mut peer = WindowManager::new();
        let parent = receiving
            .create_window(None, 23, 117, 801, 100, effects)
            .unwrap();
        let foreign = peer
            .create_window(None, 41, 211, 1001, 100, effects)
            .unwrap();
        assert!(!parent.borrow().is_hidden());
        assert!(!foreign.borrow().is_hidden());
    });
}

#[test]
fn native_popup_uses_each_receiving_clients_loaded_text() {
    use game_client::core::GameClient;
    use game_client::gui::gadgets::{ListBox, ListBoxItemData};
    use game_client::gui::WindowWidget;
    use game_engine::common::name_key_generator::NameKeyGenerator;

    fn setup(client: &mut GameClient) {
        let id = NameKeyGenerator::name_to_key("Diplomacy.wnd:ListboxSolo") as i32;
        client.window_manager_owner().with_manager(|manager| {
            WindowEventEffects::with_standalone(|effects| {
                let window = manager.create_window(None, 0, 0, 200, 80, effects).unwrap();
                let mut window = window.borrow_mut();
                window.set_id(id);
                window.set_widget(WindowWidget::ListBox(ListBox::new(id as u32, 0, 0, 200, 80)));
            });
        });
    }

    fn show(client: &mut GameClient) -> String {
        client.with_window_event_effects(None, |effects| {
            effects.with_manager(|manager, effects| {
                effects.with_diplomacy(|diplomacy| {
                    diplomacy.update_briefing_text_translated("", "", true, manager);
                });
            });
        });
        let owner = client.window_manager_owner();
        let mut messages = Vec::new();
        let (_, snow, _, _, effects, stream, text) =
            client.split_frame_snow_drawable_script_ui_and_hints(&mut messages);
        let mut bar = ControlBar::new();
        let mut ui = NativeScriptUi::new(snow, &mut bar, owner)
            .with_window_effects(effects, stream)
            .with_game_text(text);
        ui.popup_message(&crate::game_logic::ScriptPopupMessageRequest {
            message: "SCRIPT:OwnedPopup".into(),
            x_percent: 0,
            y_percent: 0,
            width: 80,
            pause: false,
            pause_music: false,
            popup_generation: 1,
        }).unwrap();
        drop(ui);
        let id = NameKeyGenerator::name_to_key("Diplomacy.wnd:ListboxSolo") as i32;
        client.window_manager_owner().with_manager(|manager| {
            let window = manager.get_window_by_id(id).unwrap();
            let window = window.borrow();
            match window.widget().unwrap() {
                WindowWidget::ListBox(listbox) => match listbox.get_item_column_data(0, 0).unwrap() {
                    ListBoxItemData::Text(text) => text.clone(),
                    _ => panic!("briefing text expected"),
                },
                _ => panic!("briefing listbox expected"),
            }
        })
    }

    let root = tempfile::tempdir().unwrap();
    let first_dir = root.path().join("first");
    let second_dir = root.path().join("second");
    let empty_dir = root.path().join("empty");
    for dir in [&first_dir, &second_dir, &empty_dir] {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("world.map"), b"").unwrap();
    }
    std::fs::write(first_dir.join("map.str"), "SCRIPT:OwnedPopup\n\"First client\"\nEND\n").unwrap();
    std::fs::write(second_dir.join("map.str"), "SCRIPT:OwnedPopup\n\"Second client\"\nEND\n").unwrap();
    let mut first = GameClient::new().unwrap();
    let mut second = GameClient::new().unwrap();
    setup(&mut first);
    setup(&mut second);
    first.admit_map_text(&first_dir.join("world.map"));
    second.admit_map_text(&second_dir.join("world.map"));
    assert_eq!(show(&mut first), "First client");
    assert_eq!(show(&mut second), "Second client");
    first.admit_map_text(&empty_dir.join("world.map"));
    assert_eq!(show(&mut first), "MISSING: 'SCRIPT:OwnedPopup'");
    assert_eq!(show(&mut second), "Second client");
    std::fs::write(first_dir.join("map.str"), "SCRIPT:OwnedPopup\n\"First reloaded\"\nEND\n").unwrap();
    first.admit_map_text(&first_dir.join("world.map"));
    assert_eq!(show(&mut first), "First reloaded");
    assert_eq!(show(&mut second), "Second client");
}
