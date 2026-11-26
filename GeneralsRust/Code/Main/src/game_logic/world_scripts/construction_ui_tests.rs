//! Ordinary dozer completion carries the native UI loan into its AI attachment.
use super::*;
use generals_sim::scripting::program::{
    Condition, ConditionType, OrCondition, Script, ScriptAction, ScriptActionType, ScriptList,
};

#[derive(Default)]
struct Ui {
    events: Vec<bool>,
}
impl crate::game_logic::ScriptUiEffects for Ui {
    fn weather_visible(&mut self, _visible: bool) {}
    fn letterbox(
        &mut self,
        _camera: &mut crate::presentation_frame::camera::SessionCamera,
        enabled: bool,
        _replay: bool,
        _upgrade_source: game_engine::common::thing::receiving_module_content::UpgradeTooltipSource<
            '_,
        >,
        _game_ending: bool,
    ) {
        self.events.push(enabled);
    }
}

/// Authored builder and scaffold: DozerAIUpdate owns BUILD (DozerAIUpdate.cpp
/// newTask/DozerActionDoActionState), so the builder carries that module.
const CONSTRUCTION_UI_INI: &str = "Object UiDozer\nKindOf = VEHICLE DOZER SELECTABLE\nLocomotor = SET_NORMAL AmericaVehicleDozerLocomotor\nBody = ActiveBody Body\nMaxHealth = 200\nInitialHealth = 200\nEnd\nBehavior = DozerAIUpdate DozerAI\nRepairHealthPercentPerSecond = 2\nEnd\nEnd\nObject UiScaffold\nKindOf = STRUCTURE SELECTABLE\nBuildCost = 100\nBuildTime = 0.001\nBody = ActiveBody Body\nMaxHealth = 900\nInitialHealth = 900\nEnd\nEnd\n";

fn fixture() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "Construction UI", true);
    player.resources.supplies = 1000;
    world.add_player(player);
    world
        .admit_object_ini_sources([(
            "construction-ui.ini".to_string(),
            CONSTRUCTION_UI_INI.to_string(),
        )])
        .expect("authored construction content admits");
    let builder = world
        .create_object_for_player("UiDozer", 0, Vec3::ZERO)
        .unwrap();
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
        .unwrap().expect("ObjectId");
    // newTask stamps m_taskOrderFrame = TheGameLogic->getFrame(), and
    // getMostRecentCommand (DozerAIUpdate.cpp:2256) admits only an order
    // frame > 0: the command is issued after the first logic frame.
    world.update();
    // DozerAIUpdate::privateResumeConstruction -> newTask(DOZER_TASK_BUILD):
    // records the request and claims the scaffold (setBuilder).
    assert!(world.resume_construction(&[builder], id).expect("resume"));
    assert_eq!(world.host_object(id).unwrap().builder_id, Some(builder));
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
    let engine = gamelogic::scripting::engine::ScriptEngine::new_in_published_world().unwrap();
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
    let mut ui = Ui::default();
    // One DozerActionDoActionState frame: BuildTime 0.001 s is one logic frame.
    world
        .fixture_build_action_frames_with_ui(&[id], 1. / 30., &mut Some(&mut ui))
        .expect("build action");
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
    // Pick -> Move -> DoAction (DozerAIUpdate.cpp:318-348) before the build
    // frame; the authored locomotor accelerates from rest to the dock.
    for _ in 0..600 {
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
