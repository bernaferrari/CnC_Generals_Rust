use game_client_rust::core::game_client::{ClientInputAuthority, GameClient};
use game_client_rust::gui::callbacks::skirmish_game_options_menu::{
    skirmish_game_options_menu_init, skirmish_game_options_menu_system,
};
use game_client_rust::gui::gadgets::CheckBox;
use game_client_rust::gui::{WindowLayout, WindowMessage, WindowWidget};
use game_engine::common::name_key_generator::NameKeyGenerator;
use std::{cell::Cell, rc::Rc};

#[test]
fn initialization_notifies_the_checkbox_owner_with_its_manager() {
    let mut client =
        GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap();
    let observed = Rc::new(Cell::new(false));
    let callback_observed = observed.clone();
    let layout = client.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            let parent = manager
                .create_window_with_id(None, 0, 0, 100, 100, 991, effects)
                .unwrap();
            parent.borrow_mut().set_system_callback_with_message_sink(
                move |window, msg, data1, data2, effects| {
                    if msg == WindowMessage::GadgetSelected {
                        effects.with_manager(|manager, _| {
                            assert!(
                                manager
                                    .get_window_by_id_with_selected(991, Some(window))
                                    .is_some()
                            )
                        });
                        callback_observed.set(true);
                    }
                    skirmish_game_options_menu_system(window, msg, data1, data2, effects)
                },
            );
            let checkbox_id = NameKeyGenerator::name_to_key(
                "SkirmishGameOptionsMenu.wnd:CheckboxLimitSuperweapons",
            ) as i32;
            let checkbox = manager
                .create_window_with_id(Some(&parent), 0, 0, 20, 20, checkbox_id, effects)
                .unwrap();
            checkbox.borrow_mut().set_owner(Some(&parent));
            checkbox
                .borrow_mut()
                .set_widget(WindowWidget::CheckBox(CheckBox::new(
                    checkbox_id as u32,
                    0,
                    0,
                    20,
                )));
            let mut layout = WindowLayout::new("Menus/SkirmishGameOptionsMenu.wnd".into());
            layout.add_window(parent);
            layout.add_window(checkbox);
            layout
        })
    });
    client.with_window_event_effects(None, |effects| {
        skirmish_game_options_menu_init(&layout, None, effects);
    });
    assert!(
        observed.get(),
        "the synchronous owner notification must run"
    );
}
