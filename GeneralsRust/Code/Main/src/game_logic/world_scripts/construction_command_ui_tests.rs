//! Real queued Build admission followed by the receiving construction callback.
//! Factory birth/payment/placement remain DozerAIUpdate.cpp:1671-1717; the
//! attachment executes at AIPlayer.cpp:118 before completion returns.
use super::super::*;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Script, ScriptAction, ScriptActionType, ScriptList,
};

#[derive(Default)]
struct ConstructionUi(Vec<bool>);
impl crate::game_logic::ScriptUiEffects for ConstructionUi {
    fn weather_visible(&mut self, _visible: bool) {}
    fn letterbox(
        &mut self,
        _camera: &mut crate::presentation_frame::camera::SessionCamera,
        enabled: bool,
        _replay: bool,
    ) {
        self.0.push(enabled);
    }
}

fn world() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "Command UI", true);
    player.resources.supplies = 1000;
    world.add_player(player);
    let mut dozer = ThingTemplate::new("CommandUiDozer");
    dozer.add_kind_of(KindOf::Dozer).set_health(200.);
    world.templates.insert(dozer.name.clone(), dozer);
    let builder = world
        .create_object_for_player("CommandUiDozer", 0, Vec3::new(-200., 0., -200.))
        .unwrap();
    let mut structure = ThingTemplate::new("CommandUiScaffold");
    structure
        .add_kind_of(KindOf::Structure)
        .set_health(900.)
        .set_cost(100, 0);
    world.templates.insert(structure.name.clone(), structure);
    (world, builder)
}

fn build(
    world: &mut GameLogic,
    builder: ObjectId,
    ui: &mut Option<&mut dyn crate::game_logic::ScriptUiEffects>,
) -> ObjectId {
    world.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::Build {
            template_name: "CommandUiScaffold".into(),
            location: Vec3::new(120., 0., 140.),
        },
        player_id: 0,
        command_id: 454,
        timestamp: std::time::UNIX_EPOCH,
        selected_units: vec![builder],
        modifier_keys: Default::default(),
    });
    world.process_commands_with_ui(ui);
    world
        .host_objects()
        .registered_values()
        .find(|object| object.template_name == "CommandUiScaffold")
        .expect("the actual queued Build must admit a scaffold")
        .id
}

fn attachment(world: &mut GameLogic, scaffold: ObjectId) {
    world
        .ai_manager
        .add_ai_player(0, Team::USA, crate::ai::AIDifficulty::Medium);
    let mut pad =
        crate::ai::AIBuildingInfo::new("CommandUiScaffold".into(), Vec3::new(120., 0., 140.), 1);
    pad.object_id = Some(scaffold);
    pad.completion = Some(crate::ai::AuthoredStructureCompletion {
        name: "CompletedCommandUiScaffold".into(),
        script: "CommandCompletionUi".into(),
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
    script.script_name = "CommandCompletionUi".into();
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
}

#[test]
fn queued_build_preserves_scaffold_then_receiving_ui_completion() {
    let (mut owner, builder) = world();
    let (mut peer, peer_builder) = world();
    assert_eq!(builder, peer_builder);
    let mut ui = ConstructionUi::default();
    let scaffold = build(&mut owner, builder, &mut Some(&mut ui));
    let peer_scaffold = build(&mut peer, peer_builder, &mut None);
    assert_eq!(scaffold, peer_scaffold);
    let object = owner.host_object(scaffold).unwrap();
    assert!(object.status.under_construction);
    assert_eq!(object.health.current, 1.);
    assert_eq!(object.construction_percent, 0.);
    assert_eq!(object.producer_id, Some(builder));
    assert_eq!(object.builder_id, Some(builder));
    assert_eq!(object.get_position(), Vec3::new(120., 0., 140.));
    assert_eq!(owner.get_player(0).unwrap().resources.supplies, 900);
    assert!(ui.0.is_empty(), "ordinary scaffold birth is not completion");
    attachment(&mut owner, scaffold);
    attachment(&mut peer, peer_scaffold);
    owner.notify_structure_construction_complete_with_ui(scaffold, None, &mut Some(&mut ui));
    assert_eq!(
        ui.0,
        [true, false],
        "the real attachment completes synchronously"
    );
    assert!(owner.session_view.pending_letterbox_visibility.is_empty());
    assert!(
        owner
            .session_view
            .letterbox_visibility_delivered_since_present
    );
    assert_eq!(
        owner.host_object(scaffold).unwrap().name,
        "CompletedCommandUiScaffold"
    );
    assert_eq!(owner.host_object(scaffold).unwrap().health.current, 675.);
    assert!(
        owner
            .host_object(scaffold)
            .unwrap()
            .construction_power_admitted
    );
    assert!(
        owner.ai_manager.ai_players[&0]
            .building_queue
            .iter()
            .find(|pad| pad.object_id == Some(scaffold))
            .expect("the completed pad remains bound to its object")
            .is_built
    );
    assert!(
        peer.host_object(peer_scaffold)
            .unwrap()
            .status
            .under_construction
    );
    assert_eq!(peer.host_object(peer_scaffold).unwrap().health.current, 1.);
    assert!(
        !peer
            .host_object(peer_scaffold)
            .unwrap()
            .construction_power_admitted
    );
    assert!(
        !peer
            .session_view
            .letterbox_visibility_delivered_since_present
    );
}
