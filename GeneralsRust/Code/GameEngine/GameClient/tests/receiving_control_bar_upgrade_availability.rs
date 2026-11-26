use std::sync::{Arc, RwLock};

use game_client_rust::gui::control_bar::{CommandAvailability, CommandOption, ControlBar};
use game_engine::common::ini::INI;
use game_engine::common::system::upgrade::UpgradeCenter;
use gamelogic::object::registry::OBJECT_REGISTRY;
use gamelogic::player::PlayerArcExt;
use gamelogic::system::engine_stores::{new_for_world, with_active_stores};
use gamelogic::upgrade::center::with_upgrade_center_mut;

#[test]
fn required_upgrade_is_resolved_from_the_receiving_bar() {
    let world = new_for_world();
    with_active_stores(&world, check_receiving_upgrade_availability);
}

fn check_receiving_upgrade_availability() {
    const OBJECT_ID: u32 = 719_201;
    const PLAYER_ID: i32 = 7;
    const NAME: &str = "Upgrade_ReceivingAvailability";
    const BUTTON: &str = "Command_ReceivingAvailability";

    // Leave a conflicting ambient definition installed: the peer must not
    // acquire an upgrade merely because another world registered it.
    with_upgrade_center_mut(|center| {
        center.new_upgrade(NAME.into());
    });
    // The receiving instance's own CommandButton catalog.
    let services = game_engine::common::common_services::CommonServices::new();
    let catalog_owner = || {
        Some(
            game_engine::common::ini::command_definitions::CommandDefinitions::from_common(
                &services,
            ),
        )
    };
    {
        let mut definitions = services.ini.control_bar.write();
        let button = definitions.new_command_button(BUTTON.to_owned());
        button.command = "STOP".into();
        button.upgrade = NAME.into();
        button.options_bits = CommandOption::NeedUpgrade as u32;
    }
    let template: Arc<dyn gamelogic::thing_template::ThingTemplate> = Arc::new(
        gamelogic::thing_template::DefaultThingTemplate::new("ReceivingAvailability".into()),
    );
    let object = Arc::new(RwLock::new(gamelogic::object::Object::new_raw(
        template,
        OBJECT_ID,
        gamelogic::common::types::ObjectStatusMaskType::none(),
        None,
    )));
    OBJECT_REGISTRY.register_object(OBJECT_ID, &object);
    {
        let __arc_bind = gamelogic::player::player_list();

        let mut players = __arc_bind.write().unwrap();
        *players = gamelogic::player::PlayerList::new();
        players.add_player(Arc::new(RwLock::new(gamelogic::player::Player::new(
            PLAYER_ID,
        ))));
    }
    let mut catalog = UpgradeCenter::new();
    catalog.new_upgrade(NAME.into());
    let empty = UpgradeCenter::new();
    let mut owner = ControlBar::new();
    let mut peer = ControlBar::new();
    owner.set_receiving_upgrade_definitions(&catalog, None);
    owner.bind_command_definitions(catalog_owner());
    peer.set_receiving_upgrade_definitions(&empty, None);
    peer.bind_command_definitions(catalog_owner());
    assert_eq!(
        owner.leftover_evaluate_named_command(BUTTON, OBJECT_ID, PLAYER_ID as u32),
        CommandAvailability::Restricted
    );
    assert_eq!(
        peer.leftover_evaluate_named_command(BUTTON, OBJECT_ID, PLAYER_ID as u32),
        CommandAvailability::Available
    );
    assert_eq!(
        owner.leftover_evaluate_named_command(BUTTON, OBJECT_ID, PLAYER_ID as u32),
        CommandAvailability::Restricted
    );
    // The same mask can be present on the player without satisfying an
    // object-type upgrade, as in C++ ControlBarCommand.cpp.
    let player = gamelogic::player::player_list()
        .read()
        .unwrap()
        .get_player(PLAYER_ID)
        .unwrap()
        .clone();
    player.add_upgrade(
        &catalog.find_upgrade(NAME).unwrap(),
        gamelogic::upgrade::UpgradeStatus::Complete,
        None,
    );
    assert_eq!(
        owner.leftover_evaluate_named_command(BUTTON, OBJECT_ID, PLAYER_ID as u32),
        CommandAvailability::Available
    );
    let mut object_catalog = UpgradeCenter::new();
    object_catalog.init();
    let object_name = "Upgrade_Veterancy_VETERAN";
    {
        let mut definitions = services.ini.control_bar.write();
        let button = definitions.new_command_button("Command_ObjectRequirement".into());
        button.command = "STOP".into();
        button.upgrade = object_name.into();
        button.options_bits = CommandOption::NeedUpgrade as u32;
    }
    let mut object_bar = ControlBar::new();
    object_bar.set_receiving_upgrade_definitions(&object_catalog, None);
    object_bar.bind_command_definitions(catalog_owner());
    let mut player_catalog = UpgradeCenter::new();
    player_catalog.new_upgrade(object_name.into());
    let mut player_bar = ControlBar::new();
    player_bar.set_receiving_upgrade_definitions(&player_catalog, None);
    player_bar.bind_command_definitions(catalog_owner());
    assert_eq!(
        object_bar.leftover_evaluate_named_command(
            "Command_ObjectRequirement",
            OBJECT_ID,
            PLAYER_ID as u32
        ),
        CommandAvailability::Restricted
    );
    assert_eq!(
        player_bar.leftover_evaluate_named_command(
            "Command_ObjectRequirement",
            OBJECT_ID,
            PLAYER_ID as u32,
        ),
        CommandAvailability::Available
    );

    const QUEUED_UPGRADE: &str = "Upgrade_ReceivingCost";
    const QUEUE_BUTTON: &str = "Command_ReceivingCost";
    {
        let mut definitions = services.ini.control_bar.write();
        let button = definitions.new_command_button(QUEUE_BUTTON.into());
        button.command = "PLAYER_UPGRADE".into();
        button.upgrade = QUEUED_UPGRADE.into();
    }
    let priced_catalog = |cost| {
        let mut catalog = catalog.clone();
        let source = format!("Upgrade {QUEUED_UPGRADE}\nBuildCost = {cost}\nEnd\n");
        INI::new()
            .with_inline_source(&source, |ini| {
                ini.read_line()?;
                catalog.parse_upgrade_definition(ini)
            })
            .unwrap();
        catalog
    };
    owner.set_receiving_upgrade_definitions(&priced_catalog(100), None);
    peer.set_receiving_upgrade_definitions(&priced_catalog(101), None);
    player.write().unwrap().get_money_mut().set_money(100);
    assert_eq!(
        owner.leftover_evaluate_named_command(QUEUE_BUTTON, OBJECT_ID, PLAYER_ID as u32),
        CommandAvailability::Available
    );
    assert_eq!(
        peer.leftover_evaluate_named_command(QUEUE_BUTTON, OBJECT_ID, PLAYER_ID as u32),
        CommandAvailability::Restricted
    );
    OBJECT_REGISTRY.unregister_object(OBJECT_ID);
}
