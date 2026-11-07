use game_client_rust::core::game_client::{ClientInputAuthority, GameClient};
use game_client_rust::gui::callbacks::skirmish_game_options_menu::{
    skirmish_game_options_menu_init, skirmish_game_options_menu_system,
};
use game_client_rust::gui::gadgets::{CheckBox, ComboBox};
use game_client_rust::gui::{GCM_SELECTED, WindowLayout, WindowMessage, WindowWidget};
use game_engine::common::name_key_generator::NameKeyGenerator;
use std::{cell::Cell, rc::Rc};

#[test]
fn initialization_and_checkbox_notifications_preserve_each_clients_lobby() {
    let _services = game_engine::common::common_services::CommonServicesScope::new();
    let world = gamelogic::system::engine_stores::new_world_services();
    gamelogic::system::engine_stores::with_world_services(&world, || {
        let mut client =
            GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap();
        let other =
            GameClient::new_with_input_authority(ClientInputAuthority::NativeHosted).unwrap();
        let observed = Rc::new(Cell::new(0usize));
        let callback_observed = observed.clone();
        let (layout, parent, checkbox, cash) = client.with_window_event_effects(None, |effects| {
            effects.with_manager(|manager, effects| {
                let parent = manager
                    .create_window_with_id(None, 0, 0, 100, 100, 991, effects)
                    .unwrap();
                parent.borrow_mut().set_system_callback_with_message_sink(
                    move |window, msg, data1, data2, effects| {
                        if msg == WindowMessage::GadgetSelected {
                            effects.with_manager(|manager, _| {
                                assert!(manager
                                    .get_window_by_id_with_selected(991, Some(window))
                                    .is_some());
                            });
                            callback_observed.set(callback_observed.get() + 1);
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
                checkbox.borrow_mut().set_widget(WindowWidget::CheckBox(CheckBox::new(
                    checkbox_id as u32,
                    0,
                    0,
                    20,
                )));
                let cash_id = NameKeyGenerator::name_to_key(
                    "SkirmishGameOptionsMenu.wnd:ComboBoxStartingCash",
                ) as i32;
                let cash = manager
                    .create_window_with_id(Some(&parent), 0, 20, 80, 20, cash_id, effects)
                    .unwrap();
                cash.borrow_mut().set_widget(WindowWidget::ComboBox(ComboBox::new(
                    cash_id as u32,
                    0,
                    20,
                    80,
                    20,
                )));
                let mut layout = WindowLayout::new("Menus/SkirmishGameOptionsMenu.wnd".into());
                layout.add_window(parent.clone());
                layout.add_window(checkbox.clone());
                layout.add_window(cash.clone());
                (layout, parent, checkbox, cash)
            })
        });
        client.with_window_event_effects(None, |effects| {
            skirmish_game_options_menu_init(&layout, None, effects);
        });
        assert!(observed.get() > 0, "the synchronous owner notification must run");
        let other_setup = other.skirmish_setup();
        {
            let mut setup = other_setup.lock().unwrap();
            setup.game_info_mut().game_info_mut().set_superweapon_restriction(1);
        }
        client.with_window_event_effects(None, |effects| {
            checkbox.borrow_mut().gadget_check_box_set_checked(false, effects);
        });
        assert_eq!(
            client.skirmish_setup().lock().unwrap().game_info().game_info()
                .get_superweapon_restriction(),
            0
        );
        assert_eq!(
            other_setup.lock().unwrap().game_info().game_info().get_superweapon_restriction(),
            1,
            "a synchronous checkbox callback must update only its own lobby"
        );
        client.with_window_event_effects(None, |effects| {
            checkbox.borrow_mut().gadget_check_box_set_checked(true, effects);
        });
        assert_eq!(
            client.skirmish_setup().lock().unwrap().game_info().game_info()
                .get_superweapon_restriction(),
            1
        );
        let notifications_before_slot_refresh = observed.get();
        let cash_id = NameKeyGenerator::name_to_key(
            "SkirmishGameOptionsMenu.wnd:ComboBoxStartingCash",
        ) as usize;
        client.with_window_event_effects(None, |effects| {
            parent.borrow_mut().send_system_message(
                WindowMessage::GadgetSelected,
                cash_id,
                0,
                effects,
            );
        });
        assert_eq!(
            observed.get(), notifications_before_slot_refresh + 1,
            "C++ slot-list refresh does not re-set the superweapon checkbox"
        );
        cash.borrow_mut().combo_box_mut().unwrap().select_item(30000);
        client.with_window_event_effects(None, |effects| {
            parent.borrow_mut().send_system_message(
                WindowMessage::User(GCM_SELECTED),
                cash_id,
                0,
                effects,
            );
        });
        assert_eq!(
            client.skirmish_setup().lock().unwrap().game_info().game_info()
                .get_starting_cash().count_money(),
            30000,
            "the real combo-selection message updates this lobby"
        );
        assert_eq!(
            client.skirmish_setup().lock().unwrap().game_info().game_info()
                .get_superweapon_restriction(),
            1
        );
    });
}
