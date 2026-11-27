//! Authored rank changes through the borrowed world and UI.
use crate::game_logic::{GameLogic, Player, Team};
use gamelogic::scripting::engine::ScriptEngine;
use generals_sim::scripting::program::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList, THIS_PLAYER,
};

use crate::assets::ReceivingObjectContent;
use crate::game_logic::{ScriptPresentationEffects, ScriptUiEffects};
use game_engine::common::ini::{get_rank_info_store_mut, ini_rank::RankInfo};
use game_engine::common::message_stream::ReceivingRankScienceUi;
use game_engine::common::rts::science::{ScienceNameFacts, ScienceStore};
#[derive(Default)]
pub(super) struct RankUi {
    pub(super) events: Vec<ReceivingRankScienceUi>,
}

impl ScriptUiEffects for RankUi {
    fn weather_visible(&mut self, _visible: bool) {}

    fn letterbox(
        &mut self,
        _camera: &mut crate::presentation_frame::camera::SessionCamera,
        _enabled: bool,
        _replay: bool,
        _upgrades: game_engine::common::thing::receiving_module_content::UpgradeTooltipSource<'_>,
        _game_ending: bool,
    ) {
    }

    fn game_text(&mut self, key: &str) -> generals_sim::GameLogicResult<String> {
        assert_eq!(key, "SCIENCE:GeneralName");
        Ok("Receiving General".into())
    }

    fn rank_science_event(&mut self, event: ReceivingRankScienceUi) {
        self.events.push(event);
    }
}

fn action(kind: ScriptActionType, player: Option<&str>, value: Option<Parameter>) -> ScriptAction {
    let mut action = ScriptAction::new(kind);
    if let Some(player) = player {
        action
            .add_parameter(Parameter::with_string(ParameterType::Side, player.into()))
            .unwrap();
    }
    if let Some(value) = value {
        action.add_parameter(value).unwrap();
    }
    action
}

fn run(world: &mut GameLogic, side: usize, actions: Vec<ScriptAction>) {
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "RankOwner".into();
    script.condition = Some(Box::new(condition));
    script.is_one_shot = true;
    for action in actions {
        script.append_action(Box::new(action));
    }
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let engine = ScriptEngine::new_in_published_world().unwrap();
    engine
        .set_script_list_for_player(side, Some(Box::new(list)))
        .unwrap();
    let mut ui = RankUi::default();
    world
        .update_local_script_engine_with_effects(
            &engine,
            ScriptPresentationEffects {
                ui: Some(&mut ui),
                ..Default::default()
            },
        )
        .unwrap();
}

fn world(name: &str) -> GameLogic {
    let mut world = GameLogic::new();
    let science = ScienceStore::new_owned();
    world
        .admit_receiving_module_content(
            ReceivingObjectContent::from_sources_with_sciences(
                Vec::<(String, String)>::new(),
                std::sync::Arc::new(ScienceNameFacts::from_store(&science)),
            )
            .unwrap(),
        )
        .unwrap();
    admit_ranks(&world);
    world.add_player(Player::new(1, Team::USA, name, false).with_script_identity(name));
    world
}

fn admit_ranks(world: &GameLogic) {
    world.with_own_services(|| {
        let mut ranks = get_rank_info_store_mut();
        ranks.init();
        for points in [0, 100, 200, 300, 400] {
            let mut rank = RankInfo::new();
            rank.skill_points_needed = points;
            ranks.add_rank(rank).unwrap();
        }
    });
}

fn integer(value: i32) -> Parameter {
    Parameter::with_int(ParameterType::Int, value)
}

#[test]
fn rank_and_experience_actions_use_the_receiving_world() {
    let mut first = world("First");
    let mut peer = world("Peer");
    // ScriptEngine.cpp:5555-5557 selects PlayerList::getNthPlayer(side).
    // These fixtures admit player 1, so its side script belongs in slot 1.
    run(
        &mut first,
        1,
        vec![
            action(
                ScriptActionType::PlayerSetRanklevellimit,
                None,
                Some(integer(2)),
            ),
            action(
                ScriptActionType::PlayerSetRanklevel,
                Some(THIS_PLAYER),
                Some(integer(5)),
            ),
        ],
    );
    run(
        &mut peer,
        1,
        vec![
            action(
                ScriptActionType::PlayerSetRanklevellimit,
                None,
                Some(integer(4)),
            ),
            action(
                ScriptActionType::PlayerSetRanklevel,
                Some(THIS_PLAYER),
                Some(integer(5)),
            ),
        ],
    );
    assert_eq!(first.get_player(1).unwrap().rank_level, 2);
    assert_eq!(peer.get_player(1).unwrap().rank_level, 4);

    run(
        &mut first,
        1,
        vec![
            action(
                ScriptActionType::PlayerAddRanklevel,
                Some(THIS_PLAYER),
                Some(integer(-1)),
            ),
            action(
                ScriptActionType::PlayerAffectReceivingExperience,
                Some(THIS_PLAYER),
                Some(Parameter::with_real(ParameterType::Real, 2.0)),
            ),
            action(
                ScriptActionType::PlayerAddSkillpoints,
                Some(THIS_PLAYER),
                Some(integer(25)),
            ),
        ],
    );
    let expected_points = game_engine::base_type::real_to_int_ceil(50.0);
    assert_eq!(first.get_player(1).unwrap().rank_level, 1);
    assert_eq!(first.get_player(1).unwrap().skill_points, expected_points);
    assert_eq!(
        first.get_player(1).unwrap().general_name,
        "Receiving General"
    );
    assert_eq!(peer.get_player(1).unwrap().rank_level, 4);
    assert_eq!(peer.get_player(1).unwrap().skill_points_modifier, 1.0);

    run(
        &mut first,
        0,
        vec![action(
            ScriptActionType::PlayerSetRanklevel,
            Some(THIS_PLAYER),
            Some(integer(5)),
        )],
    );
    assert_eq!(first.get_player(1).unwrap().rank_level, 1);
    peer.reset();
    assert_eq!(first.rank_level_limit(), 2);
    assert_eq!(first.get_player(1).unwrap().skill_points, expected_points);

    let mut numeric = GameLogic::new();
    admit_ranks(&numeric);
    numeric.add_player(Player::new(1, Team::USA, "Numeric", false).with_script_identity("Numeric"));
    run(
        &mut numeric,
        1,
        vec![action(
            ScriptActionType::PlayerAddSkillpoints,
            Some(THIS_PLAYER),
            Some(integer(125)),
        )],
    );
    assert_eq!(numeric.get_player(1).unwrap().rank_level, 2);
    assert_eq!(
        numeric.get_player(1).unwrap().skill_points,
        game_engine::base_type::real_to_int_ceil(125.0),
    );
}
