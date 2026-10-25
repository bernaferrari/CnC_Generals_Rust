use super::super::*;
use super::*;
use gamelogic::upgrade::UpgradeTemplate;

#[test]
fn receiving_upgrade_tooltip_uses_own_purchased_conflicting_and_cost_observations(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
        module_path!(),
        "receiving_upgrade_tooltip_uses_own_purchased_conflicting_and_cost_observations",
    ) {
        return;
    }
    let mut manager = WindowManager::new();
    let parent = manager
        .create_window(None, 0, 0, 500, 150, effects)
        .unwrap();
    parent
        .borrow_mut()
        .set_id(NameKeyGenerator::name_to_key("ReceivingUpgradeTooltip:Parent") as i32);
    let description = manager
        .create_window(Some(&parent), 0, 0, 480, 120, effects)
        .unwrap();
    description
        .borrow_mut()
        .set_id(NameKeyGenerator::name_to_key(
            "ControlBarPopupDescription.wnd:StaticTextDescription",
        ) as i32);
    let cost = manager
        .create_window(Some(&parent), 0, 0, 80, 20, effects)
        .unwrap();
    cost.borrow_mut().set_id(NameKeyGenerator::name_to_key(
        "ControlBarPopupDescription.wnd:StaticTextCost",
    ) as i32);
    let layout = Rc::new(RefCell::new(WindowLayout::new(
        "ReceivingUpgradeTooltip".into(),
    )));
    layout.borrow_mut().add_window(parent);
    let mut authored = IniCommandButton::default();
    authored.name = "ReceivingUpgradeCommand".into();
    authored.upgrade = "Upgrade_ReceivingTooltip".into();
    authored.command = "PLAYER_UPGRADE".into();
    authored.purchased_label = "ReceivingPurchased".into();
    authored.conflicting_label = "ReceivingConflict".into();
    authored.descriptive_text = "ReceivingBase".into();
    let mut template = UpgradeTemplate::new("Upgrade_ReceivingTooltip".into());
    template.set_cost(321);
    let template = Arc::new(template);
    let mut window = GameWindow::new();
    window.set_user_data(LiveCommandButton {
        command_name: authored.name.clone(),
        upgrade: authored.upgrade.clone(),
        ..Default::default()
    });
    for (completed, object_completed, affected, object_button, expected) in [
        (true, false, None, false, "ReceivingPurchased"),
        (false, true, Some(false), true, "ReceivingPurchased"),
        (false, false, Some(false), true, "ReceivingConflict"),
        (false, false, Some(true), true, "ReceivingBase"),
        (false, false, Some(false), false, "ReceivingBase"),
    ] {
        let mut command = authored.clone();
        if object_button {
            command.command = "OBJECT_UPGRADE".into();
        }
        let observe = |name: &str, upgrade: &str| {
            assert_eq!(name, command.name);
            assert_eq!(upgrade, command.upgrade);
            UpgradeTooltipObservation {
                command: Some(command.clone()),
                template: Some(Arc::clone(&template)),
                completed,
                object_completed,
                object_affected: affected,
            }
        };
        let source = UpgradeTooltipSource::Receiving(&observe);
        let (command, status) = resolve_command_button_from_source(&window, source).unwrap();
        populate_layout_for_command(
            &layout,
            &command,
            status,
            &mut TooltipState::default(),
            &mut manager,
            source,
        );
        assert_eq!(description.borrow().get_text(), GameText::fetch(expected));
        if expected == "ReceivingBase" {
            assert_eq!(
                cost.borrow().get_text(),
                format_template(&GameText::fetch("TOOLTIP:Cost"), &["321".into()])
            );
        }
    }
    let missing = |_: &str, _: &str| UpgradeTooltipObservation {
        command: None,
        template: None,
        completed: true,
        object_completed: true,
        object_affected: Some(false),
    };
    assert!(
        resolve_command_button_from_source(&window, UpgradeTooltipSource::Receiving(&missing))
            .is_none()
    );
    let missing_upgrade = |_: &str, _: &str| UpgradeTooltipObservation {
        command: Some(authored.clone()),
        template: None,
        completed: true,
        object_completed: true,
        object_affected: Some(false),
    };
    let source = UpgradeTooltipSource::Receiving(&missing_upgrade);
    let (command, status) = resolve_command_button_from_source(&window, source).unwrap();
    populate_layout_for_command(
        &layout,
        &command,
        status,
        &mut TooltipState::default(),
        &mut manager,
        source,
    );
    assert_eq!(
        description.borrow().get_text(),
        GameText::fetch("ReceivingBase")
    );
}
