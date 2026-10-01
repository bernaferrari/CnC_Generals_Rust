// Local live-host fixture for ControlBarCommand.cpp:329-343. C++ reads the
// selected object's controlling Player; these selections belong to the local
// player. RequiredScience, rather than ScienceVec, supplies the hide source.
#[test]
fn presentation_input_owner_sciences_do_not_cross_bars() {
    use game_engine::common::rts::science::{
        ScienceInfo, get_science_store_mut, init_science_store,
    };
    const SCIENCE: i32 = 2_147_400_001;
    if get_science_store().is_none() {
        init_science_store();
    }
    get_science_store_mut()
        .unwrap()
        .add_science(ScienceInfo::new(SCIENCE, "ScienceOwnerIsolation"));
    let mut first = ControlBar::new();
    let mut second = ControlBar::new();
    first.apply_presentation_science_hide_names(&["scienceownerisolation".to_owned()]);
    second.apply_presentation_science_hide_names(&[]);
    assert!(
        first.presentation_player_has_required_science(SCIENCE),
        "the first bar's science remains owned after the second bar receives its inputs"
    );
    assert!(!second.presentation_player_has_required_science(SCIENCE));
    first.apply_presentation_science_hide_names(&[]);
    second.apply_presentation_science_hide_names(&["ScienceOwnerIsolation".to_owned()]);
    assert!(
        !first.presentation_player_has_required_science(SCIENCE),
        "the first bar must not inherit the second bar's science"
    );
    assert!(second.presentation_player_has_required_science(SCIENCE));
}

#[test]
fn presentation_input_owner_tooltip_uses_bound_window_inputs() {
    let mut first = ControlBar::new();
    let mut second = ControlBar::new();
    let command = CommandButton {
        command_name: "OwnerIsolationBuild".to_owned(),
        object: "OwnerIsolationTank".to_owned(),
        ..CommandButton::default()
    };
    let first_window = named_window("OwnerIsolationFirst");
    let second_window = named_window("OwnerIsolationSecond");
    first.apply_presentation_can_make(&[("ownerisolationtank".to_owned(), 2)]);
    first.set_control_command(&first_window, &command);
    let expected = crate::gui::gui_callbacks::control_bar_popup_description::presentation_input_owner_tooltip_text(&first_window.borrow());
    assert!(
        !expected.is_empty(),
        "the authored CanMake denial appears in the tooltip"
    );
    second.apply_presentation_can_make(&[("OwnerIsolationTank".to_owned(), 4)]);
    second.set_control_command(&second_window, &command);
    assert_eq!(crate::gui::gui_callbacks::control_bar_popup_description::presentation_input_owner_tooltip_text(&first_window.borrow()), expected, "a second game's queue denial must not rewrite the first window's money denial");
    assert_ne!(crate::gui::gui_callbacks::control_bar_popup_description::presentation_input_owner_tooltip_text(&second_window.borrow()), expected);
    second.apply_presentation_can_make(&[]);
    assert_eq!(crate::gui::gui_callbacks::control_bar_popup_description::presentation_input_owner_tooltip_text(&first_window.borrow()), expected, "clearing the other bar leaves the bound snapshot unchanged");

    // A change to the driving bar takes effect when that window is rebound;
    // the old typed userdata remains immutable until then.
    first.apply_presentation_can_make(&[("OwnerIsolationTank".to_owned(), 5)]);
    assert_eq!(crate::gui::gui_callbacks::control_bar_popup_description::presentation_input_owner_tooltip_text(&first_window.borrow()), expected);
    first.set_control_command(&first_window, &command);
    assert_ne!(crate::gui::gui_callbacks::control_bar_popup_description::presentation_input_owner_tooltip_text(&first_window.borrow()), expected);
}

#[test]
fn presentation_input_owner_constructor_and_reset_are_isolated() {
    let mut first = ControlBar::new();
    first.apply_presentation_can_make(&[("FirstTank".to_owned(), 2)]);
    first.apply_presentation_science_hide_names(&["FirstScience".to_owned()]);
    let mut second = ControlBar::new();
    assert!(second.presentation_can_make.is_empty());
    assert!(second.presentation_unlocked_sciences.is_empty());
    second.apply_presentation_can_make(&[("SecondTank".to_owned(), 4)]);
    second.apply_presentation_science_hide_names(&["SecondScience".to_owned()]);
    SubsystemInterface::reset(&mut second).unwrap();
    assert!(second.presentation_can_make.is_empty());
    assert!(second.presentation_unlocked_sciences.is_empty());
    assert_eq!(first.presentation_can_make_status("firsttank"), Some(2));
    assert_eq!(first.presentation_unlocked_sciences, ["FirstScience"]);
}

#[test]
fn presentation_input_owner_availability_tick_refreshes_bound_tooltip_without_rebinding() {
    use crate::gui::gui_callbacks::control_bar_popup_description::presentation_input_owner_tooltip_text;
    let command = CommandButton {
        command_name: "OwnerTickBuildCommand".to_owned(),
        object: "OwnerTickTank".to_owned(),
        ..CommandButton::default()
    };
    // Exercise the authored-INI resolver branch too: it must not discard the
    // specific live window's optional status when recovering the definition.
    game_engine::common::ini::ini_command_button::get_control_bar_mut()
        .unwrap()
        .new_command_button(command.command_name.clone())
        .object = command.object.clone();
    let window = crate::gui::with_window_manager(|wm| {
        wm.destroy_all_windows();
        let win = wm.create_window(None, 0, 0, 10, 10).unwrap();
        win.borrow_mut().set_name("ControlBar.wnd:ButtonCommand01");
        win.borrow_mut().hide(false).unwrap();
        win
    });
    let mut bar = ControlBar::new();
    bar.apply_presentation_can_make(&[(command.object.clone(), 2)]);
    bar.set_control_command(&window, &command);
    let initial = presentation_input_owner_tooltip_text(&window.borrow());
    let command_string_pointer = window
        .borrow()
        .get_user_data::<CommandButton>()
        .unwrap()
        .command_name
        .as_ptr();
    bar.apply_presentation_can_make(&[(command.object.clone(), 4)]);
    let availability = HashMap::from([(
        command.command_name.clone(),
        (CommandAvailability::Restricted, None),
    )]);
    bar.apply_command_availability_to_windows(std::slice::from_ref(&command), &availability);
    assert_ne!(
        presentation_input_owner_tooltip_text(&window.borrow()),
        initial
    );
    assert_eq!(
        window
            .borrow()
            .get_user_data::<CommandButton>()
            .unwrap()
            .presentation_can_make_status,
        Some(4)
    );
    assert_eq!(
        window
            .borrow()
            .get_user_data::<CommandButton>()
            .unwrap()
            .command_name
            .as_ptr(),
        command_string_pointer,
        "availability updates only the ordinal, without cloning command strings"
    );
    bar.apply_presentation_can_make(&[]);
    bar.apply_command_availability_to_windows(std::slice::from_ref(&command), &availability);
    assert!(
        presentation_input_owner_tooltip_text(&window.borrow()).is_empty(),
        "missing frozen status must not reuse the previous denial"
    );
}

#[test]
fn presentation_input_owner_science_preserves_cpp_exceptions_and_player_precedence() {
    use game_engine::common::rts::science::{
        ScienceInfo, get_science_store_mut, init_science_store,
    };
    use gamelogic::object::special_power_template::{
        SpecialPowerTemplate, get_special_power_store_mut,
    };
    use gamelogic::player::{Player, PlayerList};
    const SCIENCE: i32 = 2_147_400_002;
    if get_science_store().is_none() {
        init_science_store();
    }
    get_science_store_mut()
        .unwrap()
        .add_science(ScienceInfo::new(SCIENCE, "ScienceOwnerRequired"));
    get_special_power_store_mut().unwrap().add_template(
        SpecialPowerTemplate::new("OwnerRequiredPower".to_owned(), 2_147_400_002)
            .with_required_science(SCIENCE),
    );
    // Preserve the authored singleton fixture, including restoration on panic.
    struct RestorePlayers(Option<PlayerList>);
    impl Drop for RestorePlayers {
        fn drop(&mut self) {
            *logic_player_list().write().unwrap() = self.0.take().unwrap();
        }
    }
    let saved = std::mem::replace(
        &mut *logic_player_list().write().unwrap(),
        PlayerList::new(),
    );
    let _restore = RestorePlayers(Some(saved));
    let mut authored = IniCommandButton::default();
    authored.name = "OwnerRequiredCommand".to_owned();
    authored.command = "SPECIAL_POWER".to_owned();
    authored.options_bits = CommandOption::NeedSpecialPowerScience as u32;
    authored.special_power_template = Some("OwnerRequiredPower".to_owned());
    authored.parsed_science_required = vec![SCIENCE + 1]; // ScienceVec is not the hide source.
    let logic = gamelogic::command_button::CommandButton::from_common(2_147_400_002, &authored);
    let common = game_engine::common::ini::ini_command_button::ControlBar::new();
    let mut bar = ControlBar::new();
    assert!(
        bar.command_from_set_slot(&common, Some(&logic))
            .button_hidden
    );
    bar.apply_presentation_science_hide_names(&["ScienceOwnerRequired".to_owned()]);
    assert!(
        !bar.command_from_set_slot(&common, Some(&logic))
            .button_hidden
    );
    for exception in [CommandType::PurchaseScience, CommandType::QueueUpgrade] {
        let mut button = ControlBar::command_from_definition(&authored);
        button.command_type = exception;
        let empty_bar = ControlBar::new();
        empty_bar.apply_need_special_power_science(&mut button, &logic);
        assert!(!button.button_hidden, "C++ exempts {exception:?}");
    }
    // Actual crate Player science ownership precedes the host fallback, even
    // when the actual player has no science and the frozen bar does have it.
    {
        let mut players = logic_player_list().write().unwrap();
        players.add_player(Player::new(0));
        players.set_local_player_index(0);
    }
    assert!(
        bar.command_from_set_slot(&common, Some(&logic))
            .button_hidden
    );
    authored.special_power_template = None;
    let no_requirement =
        gamelogic::command_button::CommandButton::from_common(2_147_400_003, &authored);
    assert!(
        !bar.command_from_set_slot(&common, Some(&no_requirement))
            .button_hidden
    );
}
