use game_engine::common::dict::Dict;
use game_engine::common::ini::INILoadType;
use game_engine::common::rts::player_template::{
    PlayerTemplateStore, get_player_template_store_mut,
};
use game_engine::common::system::DataChunkOutput;
use game_engine::common::well_known_keys::{
    key_player_display_name, key_player_faction, key_player_is_human, key_player_name,
};
use generals_main::game_logic::{GameLogic, GameMode, PlayerTemplateIdentity};

struct ForeignRegistryEntry {
    id: u32,
    installed: gamelogic::object::registry::SharedObjectHandle,
    previous: Option<gamelogic::object::registry::SharedObjectHandle>,
}

impl Drop for ForeignRegistryEntry {
    fn drop(&mut self) {
        use gamelogic::object::registry::OBJECT_REGISTRY;
        if OBJECT_REGISTRY
            .get_object(self.id)
            .is_some_and(|current| std::sync::Arc::ptr_eq(&current, &self.installed))
        {
            if let Some(previous) = &self.previous {
                OBJECT_REGISTRY.register_object(self.id, previous);
            } else {
                OBJECT_REGISTRY.unregister_object(self.id);
            }
        }
    }
}

fn definitions(money: u32) -> PlayerTemplateStore {
    let mut store = PlayerTemplateStore::new_owned();
    store
        .apply_source(
            &format!(
                "PlayerTemplate FactionAmerica\n Side = America\n StartMoney = {money}\n StartingBuilding = AmericaCommandCenter\nEnd\n"
            ),
            INILoadType::Overwrite,
        )
        .unwrap();
    store
}

fn world(start_money: u32, map: &std::path::Path) -> GameLogic {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([(
            "Selected.ini".into(),
            "Object SelectedObject\n KindOf = SELECTABLE VEHICLE\n CommandSet = SharedSet\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n\
             Object OwnedBuilder\n KindOf = SELECTABLE VEHICLE DOZER\n CommandSet = SharedBuilderSet\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = DozerAIUpdate BuilderAI\n End\nEnd\n\
             Object OwnedSite\n KindOf = STRUCTURE SELECTABLE IMMOBILE\n BuildCost = 300\n BuildTime = 1\n Body = ActiveBody BodyTag\n MaxHealth = 500\n InitialHealth = 500\n End\nEnd\n\
             Object PeerSite\n KindOf = STRUCTURE SELECTABLE IMMOBILE\n BuildCost = 300\n BuildTime = 1\n Body = ActiveBody BodyTag\n MaxHealth = 500\n InitialHealth = 500\n End\nEnd\n\
             Object ShortcutOwner\n KindOf = STRUCTURE SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = SpecialAbility SourcePower\n SpecialPowerTemplate = SpecialPowerSpySatellite\n End\nEnd\n".into(),
        )])
        .unwrap();
    world
        .admit_player_template_definitions(definitions(start_money))
        .unwrap();
    let identity = PlayerTemplateIdentity {
        template_name: "FactionAmerica".into(),
        template_index: Some(0),
    };
    assert!(world.start_new_game_with_player_template(GameMode::SinglePlayer, 0, identity));
    assert_eq!(world.get_player(0).unwrap().resources.supplies, start_money);
    assert!(world.load_map(map.to_str().unwrap()));
    world
}

#[test]
fn player_templates_follow_the_receiving_world_through_map_reset_and_save() {
    let mut foreign = PlayerTemplateStore::new_owned();
    foreign.apply_source(
        "PlayerTemplate Other\n Side = China\nEnd\nPlayerTemplate FactionAmerica\n Side = America\n StartMoney = 9999\n StartingBuilding = AmericaCommandCenter\nEnd\n",
        INILoadType::Overwrite,
    ).unwrap();
    let previous = std::mem::replace(&mut *get_player_template_store_mut(), foreign);
    let mut map = DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 3);
    for value in [21, 21, 1, 441] {
        map.write_int(value);
    }
    for _ in 0..441 {
        map.write_byte(8);
    }
    map.close_data_chunk();
    map.open_data_chunk("SidesList", 2);
    map.write_int(2);
    let mut neutral = Dict::new();
    neutral.set_ascii_string(key_player_name(), "");
    neutral.set_ascii_string(key_player_faction(), "FactionCivilian");
    let mut human = Dict::new();
    human.set_ascii_string(key_player_name(), "Owner");
    human.set_ascii_string(key_player_faction(), "FactionAmerica");
    human.set_unicode_string(key_player_display_name(), "Owner");
    human.set_bool(key_player_is_human(), true);
    for side in [neutral, human] {
        map.write_dict(&side);
        map.write_int(0);
    }
    map.write_int(0);
    map.close_data_chunk();
    map.open_data_chunk("ObjectsList", 3);
    map.close_data_chunk();
    let bytes = map.into_ckmp_bytes();
    let root = tempfile::tempdir().unwrap();
    let create_map = |name: &str, money: u32| {
        let dir = root.path().join(name);
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("Owner.map");
        std::fs::write(&path, &bytes).unwrap();
        std::fs::write(
            dir.join("Map.ini"),
            format!(
                "Science SCIENCE_Root\n IsGrantable = Yes\nEnd\n\
                 Science SCIENCE_Shared\n PrerequisiteSciences = SCIENCE_Root\n SciencePurchasePointCost = {}\nEnd\n\
                 SpecialPower SharedPower\n Enum = SPECIAL_SPY_SATELLITE\n RequiredScience = SCIENCE_Shared\nEnd\n\
                 SpecialPower SpecialPowerSpySatellite\n Enum = SPECIAL_SPY_SATELLITE\n ShortcutPower = Yes\nEnd\n\
                 CommandButton OwnerShortcut\n Command = SPECIAL_POWER_FROM_SHORTCUT\n SpecialPower = SpecialPowerSpySatellite\nEnd\n\
                 CommandButton OrdinaryPower\n Command = SPECIAL_POWER\n SpecialPower = SpecialPowerSpySatellite\nEnd\n\
                 CommandButton BuyShared\n Command = PURCHASE_SCIENCE\n Science = SCIENCE_Shared\n ButtonImage = SharedRankImage\n TextLabel = SharedRankLabel\nEnd\n\
                 CommandButton UseShared\n Command = SPECIAL_POWER\n SpecialPower = SharedPower\n Science = SCIENCE_Shared\n Options = NEED_SPECIAL_POWER_SCIENCE OK_FOR_MULTI_SELECT\n TextLabel = BasePower\nEnd\n\
                 CommandButton StopSelected\n Command = STOP\n Options = OK_FOR_MULTI_SELECT\nEnd\n\
                 CommandButton AttackMoveSelected\n Command = ATTACK_MOVE\n Options = OK_FOR_MULTI_SELECT\nEnd\n\
                 CommandButton SelectOwned\n Command = SELECT_ALL_UNITS_OF_TYPE\n Object = SelectedObject\nEnd\n\
                 CommandButton SharedBuild\n Command = DOZER_CONSTRUCT\n Object = {}\nEnd\n\
                 CommandSet Rank1\n 1 = BuyShared\nEnd\nCommandSet Rank3\nEnd\nCommandSet Rank8\nEnd\n\
                 CommandSet SharedSet\n 1 = UseShared\n 2 = StopSelected\n 3 = AttackMoveSelected\nEnd\n\
                 CommandSet SharedBuilderSet\n 1 = SharedBuild\nEnd\n\
                 CommandSet ShortcutSet\n 1 = SelectOwned\nEnd\n\
                 PlayerTemplate FactionAmerica\n StartMoney = {money}\n IntrinsicSciences = SCIENCE_Root\n PurchaseScienceCommandSetRank1 = Rank1\n PurchaseScienceCommandSetRank3 = Rank3\n PurchaseScienceCommandSetRank8 = Rank8\n SpecialPowerShortcutCommandSet = ShortcutSet\n SpecialPowerShortcutWinName = OwnerShortcut.wnd\n SpecialPowerShortcutButtonCount = 1\nEnd\n",
                if money == 1300 { 1 } else { 5 },
                if money == 1300 { "OwnedSite" } else { "PeerSite" },
            ),
        )
        .unwrap();
        path
    };
    let first_map = create_map("first", 1300);
    let peer_map = create_map("peer", 2400);
    let mut first = world(1200, &first_map);
    let mut peer = world(2200, &peer_map);
    assert_eq!(first.get_player(1).unwrap().resources.supplies, 1300);
    assert_eq!(peer.get_player(1).unwrap().resources.supplies, 2400);
    let selected = first
        .create_object_for_player("SelectedObject", 1, glam::Vec3::ZERO)
        .unwrap();
    first
        .get_player_mut(1)
        .unwrap()
        .selected_objects
        .push(selected);
    let object = first.host_object_mut(selected).unwrap();
    object.selected = true;
    object.status.selected = true;
    let first_frame =
        generals_main::presentation_frame::PresentationFrame::build_from_logic(&mut first, 1);
    assert_eq!(first_frame.selected_command_set_name(), Some("SharedSet"));
    let peer_frame =
        generals_main::presentation_frame::PresentationFrame::build_from_logic(&mut peer, 1);
    // A foreign Core object under the same numeric registry key must not
    // decide this host object's selection or command set.
    let foreign_object = gamelogic::object::Object::new(
        std::sync::Arc::new(gamelogic::common::DefaultThingTemplate::new(
            "Foreign".into(),
        )),
        gamelogic::common::types::ObjectStatusMaskType::empty(),
        None,
    )
    .unwrap();
    assert!(!foreign_object.read().unwrap().is_locally_controlled());
    foreign_object.write().unwrap().set_script_status(
        gamelogic::object::ObjectScriptStatusBit::ScriptDisabled,
        true,
    );
    let foreign_entry = ForeignRegistryEntry {
        id: selected.0,
        previous: gamelogic::object::registry::OBJECT_REGISTRY.get_object(selected.0),
        installed: foreign_object,
    };
    gamelogic::object::registry::OBJECT_REGISTRY
        .register_object(selected.0, &foreign_entry.installed);
    let mut first_bar = game_client::gui::control_bar::ControlBar::new();
    let mut peer_bar = game_client::gui::control_bar::ControlBar::new();
    let actions = |path: &std::path::Path| {
        use game_engine::common::rts::science::{ScienceNameFacts, ScienceStore};
        use game_engine::common::thing::receiving_module_content::ReceivingActionDefinitions;
        let source = std::fs::read_to_string(path.parent().unwrap().join("Map.ini")).unwrap();
        let mut sciences = ScienceStore::new_owned();
        sciences.apply_override_text(&source).unwrap();
        let mut actions = ReceivingActionDefinitions::default();
        actions
            .apply_source(&source, &ScienceNameFacts::from_store(&sciences))
            .unwrap();
        std::sync::Arc::new(actions)
    };
    let upgrades = gamelogic::upgrade::center::UpgradeCenter::new();
    first_bar.set_receiving_upgrade_definitions(&upgrades, Some(actions(&first_map)));
    peer_bar.set_receiving_upgrade_definitions(&upgrades, Some(actions(&peer_map)));
    let mut first_client = game_client::core::GameClient::new().unwrap();
    let mut peer_client = game_client::core::GameClient::new().unwrap();
    first_client
        .set_in_game_ui_subsystem(game_client::core::subsystems::InGameUISubsystem::default());
    peer_client
        .set_in_game_ui_subsystem(game_client::core::subsystems::InGameUISubsystem::default());
    first_client.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            first_frame.apply_to_control_bar(&mut first_bar, manager, effects)
        });
    });
    peer_client.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            peer_frame.apply_to_control_bar(&mut peer_bar, manager, effects)
        });
    });
    let first_button = &first_bar.get_science_state().rank1_buttons[0];
    let peer_button = &peer_bar.get_science_state().rank1_buttons[0];
    assert_eq!(first_button.command_name, "BuyShared");
    assert_eq!(peer_button.command_name, "BuyShared");
    assert!(first_button.is_enabled && !first_button.is_hidden);
    assert!(!peer_button.is_enabled && !peer_button.is_hidden);
    assert_eq!(
        first_bar.get_special_power_shortcuts()[0].command_name,
        "SelectOwned"
    );
    assert!(!first_bar.get_special_power_shortcuts()[0].is_hidden);
    assert!(peer_bar.get_special_power_shortcuts()[0].is_hidden);
    assert_eq!(
        first_bar.command_availability("StopSelected"),
        Some(game_client::gui::control_bar::CommandAvailability::Available),
    );
    let selected_command = first_bar
        .get_context()
        .available_commands
        .iter()
        .find(|command| command.command_name == "UseShared")
        .unwrap();
    assert!(selected_command.button_hidden);
    first.queue_command(generals_main::command_system::GameCommand {
        command_type: generals_main::command_system::CommandType::PurchaseScience {
            science_name: "SCIENCE_Shared".into(),
        },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::UNIX_EPOCH,
        selected_units: Vec::new(),
        modifier_keys: generals_main::command_system::ModifierKeys::default(),
    });
    first.process_commands();
    assert!(
        first
            .get_player(1)
            .unwrap()
            .has_unlocked_science("SCIENCE_Shared")
    );
    let purchased_frame =
        generals_main::presentation_frame::PresentationFrame::build_from_logic(&mut first, 1);
    first_client.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            purchased_frame.apply_to_control_bar(&mut first_bar, manager, effects)
        });
    });
    assert!(first_bar.get_science_state().rank1_buttons[0].is_purchased);
    assert!(!peer_bar.get_science_state().rank1_buttons[0].is_purchased);
    let selected_command = first_bar
        .get_context()
        .available_commands
        .iter()
        .find(|command| command.command_name == "UseShared")
        .unwrap();
    assert!(!selected_command.button_hidden);
    assert_eq!(selected_command.button_image, "SharedRankImage");
    assert_eq!(selected_command.text_label, "SharedRankLabel");

    // CommandSet::getCommandButton observes this world's script overrides
    // for both the selected object's commands and purchase/rank artwork.
    first.set_control_bar_override("SharedSet", 1, None);
    first.set_control_bar_override("Rank1", 0, None);
    let overridden_frame =
        generals_main::presentation_frame::PresentationFrame::build_from_logic(&mut first, 1);
    first_client.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            overridden_frame.apply_to_control_bar(&mut first_bar, manager, effects)
        });
    });
    assert!(first_bar.get_context().available_commands[1].button_hidden);
    assert!(first_bar.get_science_state().rank1_buttons[0].is_hidden);
    assert_eq!(
        first_bar.get_context().available_commands[0].text_label,
        "BasePower"
    );
    assert!(!peer_bar.get_science_state().rank1_buttons[0].is_hidden);
    first.set_control_bar_override("SharedSet", 1, Some("StopSelected"));
    first.set_control_bar_override("Rank1", 0, Some("BuyShared"));

    // Shortcut source selection is a live, per-world operation. It is not
    // the selected object, and another world's matching IDs cannot supply it.
    let peer_selected = peer
        .create_object_for_player("SelectedObject", 1, glam::Vec3::ZERO)
        .unwrap();
    assert_eq!(selected, peer_selected);
    let source = first
        .create_object_for_player("ShortcutOwner", 1, glam::Vec3::ZERO)
        .unwrap();
    let peer_source = peer
        .create_object_for_player("ShortcutOwner", 1, glam::Vec3::ZERO)
        .unwrap();
    assert_eq!(source, peer_source);
    let power = generals_main::command_system::SpecialPowerType::SpySatellite;
    first
        .host_object_mut(source)
        .unwrap()
        .set_special_power_ready_seconds(&power, 10.0);
    assert!(
        first
            .shortcut_command_source(1, "OwnerShortcut", "SpecialPowerSpySatellite")
            .is_err()
    );
    assert_eq!(
        peer.shortcut_command_source(1, "OwnerShortcut", "SpecialPowerSpySatellite"),
        Ok(Some(peer_source))
    );
    first
        .host_object_mut(source)
        .unwrap()
        .set_special_power_ready_seconds(&power, 0.0);
    assert_eq!(
        first.shortcut_command_source(1, "OwnerShortcut", "SpecialPowerSpySatellite"),
        Ok(Some(source))
    );
    first.host_object_mut(source).unwrap().status.sold = true;
    assert!(
        first
            .shortcut_command_source(1, "OwnerShortcut", "SpecialPowerSpySatellite")
            .is_err()
    );
    first.host_object_mut(source).unwrap().status.sold = false;
    assert_eq!(
        first.shortcut_command_source(1, "OrdinaryPower", "SpecialPowerSpySatellite"),
        Ok(None)
    );
    assert!(
        first
            .shortcut_command_source(1, "OwnerShortcut", "WrongPower")
            .is_err()
    );
    assert_eq!(
        first.get_player(1).unwrap().selected_objects,
        vec![selected]
    );
    assert!(peer.get_player(1).unwrap().selected_objects.is_empty());

    let second = first
        .create_object_for_player("SelectedObject", 1, glam::Vec3::ZERO)
        .unwrap();
    first
        .get_player_mut(1)
        .unwrap()
        .selected_objects
        .push(second);
    let second_object = first.host_object_mut(second).unwrap();
    second_object.selected = true;
    second_object.status.selected = true;
    first
        .host_object_mut(selected)
        .unwrap()
        .status
        .disabled_paralyzed = true;
    let multi_frame =
        generals_main::presentation_frame::PresentationFrame::build_from_logic(&mut first, 1);
    first_client.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            multi_frame.apply_to_control_bar(&mut first_bar, manager, effects)
        });
    });
    assert!(
        first_bar
            .get_context()
            .available_commands
            .iter()
            .any(|command| { command.command_name == "UseShared" && !command.button_hidden })
    );
    assert_eq!(
        first_bar.command_availability("AttackMoveSelected"),
        Some(game_client::gui::control_bar::CommandAvailability::Available),
        "the healthy second selected object can still receive the command",
    );
    first.host_object_mut(second).unwrap().command_set_override = Some("MissingSet".into());
    let missing_frame =
        generals_main::presentation_frame::PresentationFrame::build_from_logic(&mut first, 1);
    first_client.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            missing_frame.apply_to_control_bar(&mut first_bar, manager, effects)
        });
    });
    assert!(
        first_bar
            .get_context()
            .available_commands
            .iter()
            .all(|command| command.button_hidden)
    );
    drop(foreign_entry);
    assert_eq!(
        get_player_template_store_mut()
            .find_template("FactionAmerica")
            .unwrap()
            .get_money()
            .count_money(),
        9999
    );
    // A lobby carries only indices. Pair each index with the name from its
    // receiving application, even while another catalog is installed globally.
    let mut info = game_engine::common::system::skirmish_game_info::SkirmishGameInfo::default();
    info.slots[0].state = game_client::SlotState::Player as i32;
    info.slots[0].player_template = 0;
    info.slots[0].name = "Owner".into();
    let launch = game_engine::common::message_stream::ClientSkirmishLaunch {
        map: first_map.to_string_lossy().into_owned(),
        game_info: info,
        local_slot: 0,
        old_factions_only: false,
        fog_of_war: true,
        game_mode_code: 2,
        max_fps: 30,
    };
    let chosen =
        generals_main::skirmish_config::config_from_skirmish_launch_in(&launch, &definitions(3700))
            .unwrap();
    assert!(matches!(&chosen.slots[0].player_template,
        generals_main::skirmish_config::SkirmishPlayerTemplateSelection::Exact { template_name, template_index: 0 } if template_name == "FactionAmerica"));
    let foreign = get_player_template_store_mut();
    let other =
        generals_main::skirmish_config::config_from_skirmish_launch_in(&launch, &foreign).unwrap();
    assert!(matches!(&other.slots[0].player_template,
        generals_main::skirmish_config::SkirmishPlayerTemplateSelection::Exact { template_name, template_index: 0 } if template_name == "Other"));
    drop(foreign);
    peer.reset();
    assert_eq!(first.get_player(1).unwrap().resources.supplies, 1300);
    let builder = generals_main::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&first).unwrap();
    let mut restored = world(1200, &first_map);
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(restored.get_player(1).unwrap().resources.supplies, 1300);
    assert_eq!(
        restored.player_template_identity(1).unwrap().template_index,
        Some(0)
    );
    assert_eq!(first.get_player(1).unwrap().resources.supplies, 1300);
    // The legacy catalog has this same faction at index 1. Real Skirmish
    // admission must validate index 0 against the receiving world instead.
    use generals_main::skirmish_config::{
        GameRulesSnapshot, SkirmishMatchConfig, SkirmishPlayerTemplateSelection,
        SkirmishSlotConfig, apply_skirmish_config,
    };
    let config = SkirmishMatchConfig {
        source_game_info: None,
        map: first_map.to_string_lossy().into_owned(),
        random_seed: 31,
        old_factions_only: false,
        rules: GameRulesSnapshot::default_rules(),
        slots: vec![SkirmishSlotConfig {
            slot_index: 0,
            is_human: true,
            is_active: true,
            player_template: SkirmishPlayerTemplateSelection::Exact {
                template_name: "FactionAmerica".into(),
                template_index: 0,
            },
            faction: "USA".into(),
            color_rgb: (0, 0, 200),
            team: 0,
            start_position: 0,
            player_name: "Owner".into(),
            ai_difficulty: None,
        }],
    };
    let mut skirmish = GameLogic::new();
    skirmish
        .admit_object_ini_sources([("Empty.ini".into(), String::new())])
        .unwrap();
    skirmish
        .admit_player_template_definitions(definitions(3700))
        .unwrap();
    apply_skirmish_config(&mut skirmish, &config).unwrap();
    assert_eq!(skirmish.get_player(0).unwrap().resources.supplies, 3700);
    assert_eq!(
        skirmish.player_template_identity(0).unwrap().template_index,
        Some(0)
    );
    assert!(skirmish.load_map(first_map.to_str().unwrap()));
    let skirmish_cash = skirmish.get_player(0).unwrap().resources.supplies;
    let skirmish_snapshot = builder.create_world_snapshot(&skirmish).unwrap();
    let mut resumed_skirmish = GameLogic::new();
    resumed_skirmish
        .admit_object_ini_sources([("Empty.ini".into(), String::new())])
        .unwrap();
    resumed_skirmish
        .admit_player_template_definitions(definitions(3700))
        .unwrap();
    apply_skirmish_config(&mut resumed_skirmish, &config).unwrap();
    assert!(resumed_skirmish.load_map(first_map.to_str().unwrap()));
    builder
        .restore_from_snapshot(&skirmish_snapshot, &mut resumed_skirmish)
        .unwrap();
    assert_eq!(
        resumed_skirmish.get_player(0).unwrap().resources.supplies,
        skirmish_cash
    );
    assert_eq!(
        resumed_skirmish
            .player_template_identity(0)
            .unwrap()
            .template_index,
        Some(0)
    );

    // Same builder/set/button names, but each Map.ini authorizes a different
    // structure. The actual DozerConstruct command must use the recipient.
    let mut build_owner = world(1200, &first_map);
    let mut build_peer = world(2200, &peer_map);
    build_owner.set_skirmish_rules(false, false, false, true, 1.0);
    build_peer.set_skirmish_rules(false, false, false, true, 1.0);
    let position = glam::Vec3::new(45.0, 0.0, 45.0);
    let owner_builder = build_owner
        .create_object_for_player("OwnedBuilder", 1, position)
        .unwrap();
    let peer_builder = build_peer
        .create_object_for_player("OwnedBuilder", 1, position)
        .unwrap();
    assert_eq!(owner_builder, peer_builder);
    assert!(
        build_owner
            .host_object(owner_builder)
            .unwrap()
            .can_construct()
    );
    let construct = |world: &mut GameLogic, selected| {
        generals_main::command_executor::CommandExecutor::new(world, 1)
            .execute_command(generals_main::command_system::GameCommand {
                command_type: generals_main::command_system::CommandType::DozerConstruct {
                    template_name: "OwnedSite".into(),
                    location: glam::Vec3::new(80.0, 0.0, 80.0),
                    orientation: 0.0,
                },
                player_id: 1,
                command_id: 10,
                timestamp: std::time::UNIX_EPOCH,
                selected_units: vec![selected],
                modifier_keys: generals_main::command_system::ModifierKeys::default(),
            })
            .unwrap()
    };
    assert_eq!(
        construct(&mut build_peer, peer_builder),
        generals_main::command_system::CommandResult::InvalidCommand
    );
    assert_eq!(build_peer.get_player(1).unwrap().resources.supplies, 2400);
    let placement = build_owner.legal_build_code_at_for_builder_ex(
        build_owner.get_player(1).unwrap().team,
        glam::Vec3::new(80.0, 0.0, 80.0),
        "OwnedSite",
        Some(owner_builder),
        0,
    );
    assert_eq!(
        placement,
        0,
        "placement code {placement}, bounds {:?}",
        build_owner.world_bounds()
    );
    assert_eq!(
        construct(&mut build_owner, owner_builder),
        generals_main::command_system::CommandResult::Success
    );
    assert_eq!(build_owner.get_player(1).unwrap().resources.supplies, 1000);
    assert_eq!(build_peer.get_player(1).unwrap().resources.supplies, 2400);
    assert!(
        build_owner
            .host_objects()
            .registered_values()
            .any(|object| {
                object.template_name == "OwnedSite"
                    && object.owner_player_id == Some(1)
                    && object.builder_id == Some(owner_builder)
                    && object.status.under_construction
            })
    );
    assert!(
        !build_peer
            .host_objects()
            .registered_values()
            .any(|object| object.template_name == "OwnedSite")
    );
    *get_player_template_store_mut() = previous;
}
