//! Authored rank actions deliver science events from the receiving native player.
use super::rank_scripts::RankUi;
use crate::assets::ReceivingObjectContent;
use crate::game_logic::{GameLogic, Player, ScriptPresentationEffects, Team};
use game_engine::common::ini::{get_rank_info_store_mut, ini_rank::RankInfo};
use game_engine::common::message_stream::ReceivingRankScienceUi;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::science::{ScienceNameFacts, ScienceStore};
use gamelogic::player::Player as CorePlayer;
use gamelogic::scripting::engine::ScriptEngine;
use gamelogic::system::rank_info::{RankDefinition, RankDefinitionMode, RankInfoStore};
use generals_sim::scripting::program::{
    Condition, ConditionType, LOCAL_PLAYER, OrCondition, Parameter, ParameterType, Script,
    ScriptAction, ScriptActionType, ScriptList,
};
use std::sync::{Arc, RwLock};

#[test]
fn rank_action_retains_science_callback_on_its_receiving_interpreter() {
    let mut world = GameLogic::new();
    let names = ["SCIENCE_ReceivingRankTwo", "SCIENCE_ReceivingRankThree"];
    let ini = format!(
        "Science {}\n IsGrantable = No\nEnd\nScience {}\n IsGrantable = No\nEnd\n",
        names[0], names[1]
    );
    let mut store = ScienceStore::new_owned();
    store
        .load_from_sources(&[("fixture-science.ini", ini.as_bytes(), false)])
        .unwrap();
    let facts = Arc::new(ScienceNameFacts::from_store(&store));
    let sciences = names.map(|name| facts.science(name).unwrap());
    world
        .admit_receiving_module_content(
            ReceivingObjectContent::from_sources_with_sciences(
                Vec::<(String, String)>::new(),
                facts,
            )
            .unwrap(),
        )
        .unwrap();
    let player_name = world.with_own_services(|| {
        let mut ranks = get_rank_info_store_mut();
        ranks.init();
        ranks.add_rank(RankInfo::new()).unwrap();
        for ((points, purchase_points), science) in [(100, 4), (200, 5)].into_iter().zip(sciences) {
            let mut rank = RankInfo::new();
            rank.skill_points_needed = points;
            rank.science_purchase_points_granted = purchase_points;
            rank.sciences_granted.push(science);
            ranks.add_rank(rank).unwrap();
        }
        NameKeyGenerator::name_to_key("RankOwner")
    });
    let legacy_player = Arc::new(RwLock::new(CorePlayer::new(0)));
    legacy_player
        .write()
        .unwrap()
        .set_player_name_key(player_name);
    world
        .core_player_roster()
        .write()
        .unwrap()
        .add_player(Arc::clone(&legacy_player));
    {
        let mut ranks = world.core_rank_info_store().write().unwrap();
        *ranks = RankInfoStore::new();
        for (level, points) in [0, 100, 200].into_iter().enumerate() {
            let mut rank = RankDefinition::new(level + 1, RankDefinitionMode::Create);
            rank.skill_points_needed = points;
            if level > 0 {
                rank.sciences_granted.push(sciences[level - 1]);
            }
            ranks.apply_rank_definition(rank).unwrap();
        }
    }
    let legacy_start = legacy_player.read().unwrap().get_rank_level();
    let legacy_cap = world
        .core_player_roster()
        .read()
        .unwrap()
        .rank_level_limit();

    world.add_player(
        Player::new(0, Team::USA, "RankOwner", false).with_script_identity("RankOwner"),
    );
    assert!(world.select_local_player(Some(0)));
    let initial_purchase_points = world.get_player(0).unwrap().science_purchase_points;
    let mut limit = ScriptAction::new(ScriptActionType::PlayerSetRanklevellimit);
    limit
        .add_parameter(Parameter::with_int(ParameterType::Int, 3))
        .unwrap();
    let mut award = ScriptAction::new(ScriptActionType::PlayerAddSkillpoints);
    award
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            LOCAL_PLAYER.into(),
        ))
        .unwrap();
    award
        .add_parameter(Parameter::with_int(ParameterType::Int, 250))
        .unwrap();
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "RankScienceCallback".into();
    script.is_one_shot = true;
    script.condition = Some(Box::new(condition));
    script.append_action(Box::new(limit));
    script.append_action(Box::new(award));
    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(script));

    let foreign = gamelogic::system::engine_stores::new_world_services();
    let mut foreign_player = CorePlayer::new(0);
    foreign_player.set_player_name_key(player_name);
    let foreign_rank = foreign_player.get_rank_level();
    let foreign_player = Arc::new(RwLock::new(foreign_player));
    foreign
        .players()
        .write()
        .unwrap()
        .add_player(Arc::clone(&foreign_player));
    let engine = ScriptEngine::new_with_services(&foreign).unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(scripts)))
        .unwrap();
    let mut ui = RankUi::default();
    gamelogic::system::engine_stores::with_world_services(&foreign, || {
        world
            .update_local_script_engine_with_effects(
                &engine,
                ScriptPresentationEffects {
                    ui: Some(&mut ui),
                    ..Default::default()
                },
            )
            .unwrap();
    });

    let player = world.get_player(0).unwrap();
    assert_eq!((player.rank_level, player.skill_points), (3, 200));
    assert!(
        names
            .iter()
            .all(|name| player.unlocked_sciences.contains(*name))
    );
    assert_eq!(engine.snapshot_xfer_tail().acquired_sciences[0], sciences);
    let rank_two = initial_purchase_points + 4;
    let rank_three = rank_two + 5;
    assert_eq!(
        ui.events,
        [
            ReceivingRankScienceUi::ScienceAdded,
            ReceivingRankScienceUi::RankChanged {
                purchase_points: rank_two
            },
            ReceivingRankScienceUi::PurchasePointsChanged {
                purchase_points: rank_two
            },
            ReceivingRankScienceUi::ScienceAdded,
            ReceivingRankScienceUi::RankChanged {
                purchase_points: rank_three
            },
            ReceivingRankScienceUi::PurchasePointsChanged {
                purchase_points: rank_three
            },
        ],
    );
    assert_eq!(world.rank_level_limit(), 3);
    assert_eq!(
        world
            .core_player_roster()
            .read()
            .unwrap()
            .rank_level_limit(),
        legacy_cap
    );
    assert_eq!(legacy_player.read().unwrap().get_rank_level(), legacy_start);
    assert_eq!(
        foreign_player.read().unwrap().get_rank_level(),
        foreign_rank
    );
    for science in sciences {
        assert!(engine.is_science_acquired(0, science, true));
        assert!(!engine.is_science_acquired(0, science, true));
    }
}
