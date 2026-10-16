//! Ordinary dozer completion carries the native UI loan into its AI attachment.
use super::*;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Script, ScriptAction, ScriptActionType, ScriptList,
};

#[derive(Default)]
struct Ui {
    events: Vec<bool>,
}
impl crate::game_logic::ScriptUiEffects for Ui {
    fn letterbox(
        &mut self,
        _camera: &mut crate::presentation_frame::camera::SessionCamera,
        enabled: bool,
        _replay: bool,
    ) {
        self.events.push(enabled);
    }
}

fn fixture() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "Construction UI", true);
    player.resources.supplies = 1000;
    world.add_player(player);
    let mut dozer = ThingTemplate::new("UiDozer");
    dozer.add_kind_of(KindOf::Dozer).set_health(200.);
    world.templates.insert(dozer.name.clone(), dozer);
    let builder = world
        .create_object_for_player("UiDozer", 0, Vec3::ZERO)
        .unwrap();
    let mut structure = ThingTemplate::new("UiScaffold");
    structure
        .add_kind_of(KindOf::Structure)
        .set_health(900.)
        .set_cost(100, 0);
    structure.build_time = 0.001;
    world.templates.insert(structure.name.clone(), structure);
    let id = world
        .create_dozer_scaffold(
            "UiScaffold",
            0,
            builder,
            Vec3::ZERO,
            0.,
            Resources {
                supplies: 100,
                power: 0,
            },
        )
        .unwrap();
    let dozer = world.host_object_mut(builder).unwrap();
    dozer.dozer_task_build_target = Some(id);
    dozer.dozer_dock_action = Some(Vec3::ZERO);
    dozer.target = Some(id);
    dozer.set_ai_state(AIState::Constructing);
    world
        .ai_manager
        .add_ai_player(0, Team::USA, crate::ai::AIDifficulty::Medium);
    let mut pad = crate::ai::AIBuildingInfo::new("UiScaffold".into(), Vec3::ZERO, 1);
    pad.object_id = Some(id);
    pad.completion = Some(crate::ai::AuthoredStructureCompletion {
        name: "CompletedUiScaffold".into(),
        script: "CompletionUi".into(),
        health_percent: 75,
        unsellable: true,
    });
    world
        .ai_manager
        .ai_players
        .get_mut(&0)
        .unwrap()
        .building_queue
        .push(pad);
    let mut script = Script::new();
    script.script_name = "CompletionUi".into();
    script.is_subroutine = true;
    script.is_one_shot = false;
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    script.condition = Some(Box::new(condition));
    let mut begin = ScriptAction::new(ScriptActionType::CameraLetterboxBegin);
    begin.next_action = Some(Box::new(ScriptAction::new(
        ScriptActionType::CameraLetterboxEnd,
    )));
    script.action = Some(Box::new(begin));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let engine = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    world.install_owned_script_engine(engine);
    (world, id)
}

#[test]
fn ordinary_completion_runs_attached_ui_synchronously_without_pending_delivery() {
    let (mut world, id) = fixture();
    let (peer, peer_id) = fixture();
    assert_eq!(id, peer_id);
    let ids: Vec<_> = world.objects.keys().copied().collect();
    let mut ui = Ui::default();
    world.update_construction_with_ui(&ids, 1., &mut Some(&mut ui));
    assert_eq!(
        ui.events,
        [true, false],
        "attachment UI finishes before construction returns"
    );
    assert!(world.session_view.pending_letterbox_visibility.is_empty());
    assert!(
        world
            .session_view
            .letterbox_visibility_delivered_since_present
    );
    assert!(world.host_object(id).unwrap().construction_power_admitted);
    assert_eq!(world.host_object(id).unwrap().health.current, 675.);
    assert_eq!(world.host_object(id).unwrap().name, "CompletedUiScaffold");
    assert!(
        world.ai_manager.ai_players[&0]
            .building_queue
            .iter()
            .find(|pad| pad.object_id == Some(id))
            .expect("the completed pad remains bound to its object")
            .is_built
    );
    assert!(
        !peer
            .host_object(peer_id)
            .unwrap()
            .construction_power_admitted
    );
    assert!(
        !peer
            .session_view
            .letterbox_visibility_delivered_since_present
    );
}

#[test]
fn admitted_native_step_keeps_ui_loan_through_ordinary_construction_phase() {
    let (mut world, id) = fixture();
    let (peer, peer_id) = fixture();
    assert_eq!(id, peer_id);
    let mut ui = Ui::default();
    world.tick_logic_frame_with_script_ui(0., None, Some(1), Some(&mut ui), |_| {}, |_| {});
    assert!(ui.events.is_empty());
    for _ in 0..3 {
        world.tick_logic_frame_with_script_ui(
            1. / 30.,
            None,
            Some(1),
            Some(&mut ui),
            |_| {},
            |_| {},
        );
        if !ui.events.is_empty() {
            break;
        }
    }
    assert_eq!(
        ui.events,
        [true, false],
        "the early interpreter phase must not consume the UI loan needed by completion"
    );
    assert!(world.session_view.pending_letterbox_visibility.is_empty());
    assert_eq!(world.host_object(id).unwrap().name, "CompletedUiScaffold");
    assert_eq!(world.host_object(id).unwrap().health.current, 675.);
    assert!(world.host_object(id).unwrap().construction_power_admitted);
    assert!(
        !peer
            .host_object(peer_id)
            .unwrap()
            .construction_power_admitted
    );
    assert!(
        !peer
            .session_view
            .letterbox_visibility_delivered_since_present
    );
}
