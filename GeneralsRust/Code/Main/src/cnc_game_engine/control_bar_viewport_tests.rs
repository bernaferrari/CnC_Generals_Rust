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

#[test]
fn script_rank_reset_uses_receiving_map_text_and_borrowed_interpreter() {
    use crate::game_logic::{GameLogic, Player, ScriptPresentationEffects, Team};
    use game_client::core::GameClient;
    use game_engine::common::ini::ini_rank::RankInfo;
    use game_engine::common::rts::science::{ScienceNameFacts, ScienceStore};
    use gamelogic::scripting::core::{
        Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
        ScriptActionType, ScriptList,
    };
    use gamelogic::scripting::engine::ScriptEngine;
    use std::sync::Arc;

    fn make_world() -> (GameLogic, [i32; 3]) {
        let mut store = ScienceStore::new_owned();
        let source = b"Science SCIENCE_OwnerBase\n IsGrantable = No\nEnd\nScience SCIENCE_OwnerTwo\n IsGrantable = No\nEnd\nScience SCIENCE_OwnerThree\n IsGrantable = No\nEnd\n";
        store.load_from_sources(&[("rank-fixture.ini", source.as_slice(), false)]).unwrap();
        let facts = Arc::new(ScienceNameFacts::from_store(&store));
        let ids = [
            facts.science("SCIENCE_OwnerBase").unwrap(),
            facts.science("SCIENCE_OwnerTwo").unwrap(),
            facts.science("SCIENCE_OwnerThree").unwrap(),
        ];
        let content = crate::assets::ReceivingObjectContent::from_sources_with_sciences(
            Vec::<(String, String)>::new(), facts,
        ).unwrap();
        let mut world = GameLogic::new();
        world.admit_receiving_module_content(content).unwrap();
        {
            let mut ranks = world.world_services.common().ini.rank.write();
            ranks.init();
            for (science, skill, points) in [(ids[0], 0, 2), (ids[1], 137, 7), (ids[2], 419, 11)] {
                let mut rank = RankInfo::new();
                rank.skill_points_needed = skill;
                rank.science_purchase_points_granted = points;
                rank.sciences_granted.push(science);
                ranks.add_rank(rank).unwrap();
            }
        }
        world.clear_all_players();
        world.add_player(Player::new(0, Team::USA, "Local", true).with_script_identity("Local"));
        world.add_player(Player::new(1, Team::China, "Target", false).with_script_identity("Target"));
        assert!(world.select_local_player(Some(0)));
        (world, ids)
    }

    fn rank(client: &mut GameClient, world: &mut GameLogic, action_type: ScriptActionType, level: i32) -> Vec<i32> {
        let mut action = ScriptAction::new(action_type);
        action.add_parameter(Parameter::with_string(ParameterType::Side, "Target".into())).unwrap();
        action.add_parameter(Parameter::with_int(ParameterType::Int, level)).unwrap();
        let mut condition = OrCondition::new();
        condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
        let mut script = Script::new();
        script.script_name = "ReceivingRank".into();
        script.condition = Some(Box::new(condition));
        script.action = Some(Box::new(action));
        script.is_one_shot = true;
        let mut list = ScriptList::new();
        list.append_script(Box::new(script));
        let engine = ScriptEngine::new().unwrap();
        engine.set_script_list_for_player(0, Some(Box::new(list))).unwrap();
        let owner = client.window_manager_owner();
        let mut messages = Vec::new();
        let (_, snow, _, _, effects, stream, text) =
            client.split_frame_snow_drawable_script_ui_and_hints(&mut messages);
        let mut bar = ControlBar::new();
        let mut ui = NativeScriptUi::new(snow, &mut bar, owner)
            .with_window_effects(effects, stream)
            .with_game_text(text);
        world.update_local_script_engine_with_effects(
            &engine,
            ScriptPresentationEffects { ui: Some(&mut ui), ..Default::default() },
        ).unwrap();
        engine.snapshot_xfer_tail().acquired_sciences[1].clone()
    }

    let root = tempfile::tempdir().unwrap();
    let a_dir = root.path().join("a");
    let b_dir = root.path().join("b");
    let empty = root.path().join("empty");
    for dir in [&a_dir, &b_dir, &empty] {
        std::fs::create_dir_all(dir).unwrap();
    }
    std::fs::write(a_dir.join("map.str"), "SCIENCE:GeneralName\n\"Alpha General\"\nEND\n").unwrap();
    std::fs::write(b_dir.join("map.str"), "SCIENCE:GeneralName\n\"Beta General\"\nEND\n").unwrap();
    let mut a_client = GameClient::new().unwrap();
    let mut b_client = GameClient::new().unwrap();
    for client in [&mut a_client, &mut b_client] {
        client.set_in_game_ui_subsystem(
            game_client::core::subsystems::InGameUISubsystem::default(),
        );
    }
    a_client.admit_map_text(&a_dir.join("world.map"));
    b_client.admit_map_text(&b_dir.join("world.map"));
    let (mut a, ids) = make_world();
    let (mut b, _) = make_world();
    let (mut foreign, _) = make_world();
    a.get_player_mut(1).unwrap().general_name = "Old Alpha".into();
    b.get_player_mut(1).unwrap().general_name = "Old Beta".into();
    foreign.get_player_mut(1).unwrap().general_name = "Foreign".into();
    let foreign_services = Arc::clone(&foreign.world_services);
    gamelogic::system::engine_stores::with_world_services(&foreign_services, || {
        assert_eq!(rank(&mut a_client, &mut a, ScriptActionType::PlayerSetRanklevel, 3), vec![ids[1], ids[2]]);
        assert_eq!(rank(&mut b_client, &mut b, ScriptActionType::PlayerSetRanklevel, 3), vec![ids[1], ids[2]]);
        assert_eq!(rank(&mut a_client, &mut a, ScriptActionType::PlayerAddRanklevel, -2), vec![ids[0], ids[0]]);
        assert_eq!(a.get_player(1).unwrap().general_name, "Alpha General");
        assert_eq!(a.get_player(1).unwrap().rank_level, 1);
        assert_eq!(a.get_player(1).unwrap().science_purchase_points, 2);
        assert_eq!(b.get_player(1).unwrap().general_name, "Old Beta");
        assert_eq!(foreign.get_player(1).unwrap().general_name, "Foreign");
        assert_eq!(rank(&mut b_client, &mut b, ScriptActionType::PlayerSetRanklevel, 2), vec![ids[0], ids[0], ids[1]]);
        assert_eq!(b.get_player(1).unwrap().general_name, "Beta General");
        a_client.admit_map_text(&empty.join("world.map"));
        rank(&mut a_client, &mut a, ScriptActionType::PlayerSetRanklevel, 2);
        rank(&mut a_client, &mut a, ScriptActionType::PlayerSetRanklevel, 1);
        assert_eq!(a.get_player(1).unwrap().general_name, "MISSING: 'SCIENCE:GeneralName'");
        assert_eq!(b.get_player(1).unwrap().general_name, "Beta General");
        assert_eq!(foreign.get_player(1).unwrap().general_name, "Foreign");
    });
}
