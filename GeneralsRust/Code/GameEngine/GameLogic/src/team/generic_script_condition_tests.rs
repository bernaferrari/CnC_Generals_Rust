//! Team generic conditions bypass ordinary root executeScript eligibility gates.
use super::*;
use crate::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, ScriptAction,
    ScriptActionType, ScriptList,
};
use crate::scripting::engine::{ScriptActionHandler, ScriptEngine, ScriptEngineHandle};

#[derive(Default)]
struct Visits(Mutex<Vec<String>>);
impl ScriptActionHandler for Visits {
    fn display_text(&self, text: &str) -> crate::GameLogicResult<()> {
        self.0.lock().unwrap().push(text.into());
        Ok(())
    }
}
fn display(text: &str) -> Box<ScriptAction> {
    let mut action = ScriptAction::new(ScriptActionType::DisplayText);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::TextString,
            text.into(),
        ))
        .unwrap();
    Box::new(action)
}
fn script(name: &str) -> Script {
    let mut script = Script::new();
    script.set_name(name.into());
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    script.condition = Some(Box::new(condition));
    script.action = Some(display(name));
    script
}
struct Local {
    factory: TeamFactory,
    prototype: Arc<TeamPrototype>,
    team: Arc<RwLock<Team>>,
    engine: ScriptEngineHandle,
    visits: Arc<Visits>,
}
impl Local {
    fn new(scripts: Vec<Script>) -> Self {
        let mut list = ScriptList::new();
        let mut prototype = TeamPrototype::new("GenericConditions".into());
        for (slot, script) in scripts.into_iter().enumerate() {
            prototype.set_generic_script(slot, script.script_name.clone().into());
            list.append_script(Box::new(script));
        }
        let prototype = Arc::new(prototype);
        let team = Arc::new(RwLock::new(Team::new("GenericConditions".into(), 81)));
        let mut factory = TeamFactory::new();
        factory
            .prototypes
            .insert("GenericConditions".into(), prototype.clone());
        factory.admit_team_instance(81, team.clone());
        let visits = Arc::new(Visits::default());
        let mut engine = ScriptEngine::new().unwrap();
        engine
            .set_script_list_for_player(0, Some(Box::new(list)))
            .unwrap();
        engine.set_action_handler(Some(visits.clone()));
        Self {
            factory,
            prototype,
            team,
            engine: ScriptEngineHandle::from_engine(engine),
            visits,
        }
    }
    fn update(&mut self) {
        self.factory.update();
        execute_pending_team_generic_script_evals_with_engine(
            self.factory.drain_pending_generic_script_evals(),
            &self.engine,
        );
    }
}
#[test]
fn actual_generic_default_disable_root_and_initially_inactive_slots_still_evaluate() {
    let mut first = script("DisableRoot");
    let mut disable = ScriptAction::new(ScriptActionType::DisableScript);
    disable
        .add_parameter(Parameter::with_string(
            ParameterType::Script,
            "LaterRoot".into(),
        ))
        .unwrap();
    disable.next_action = Some(display("DisableRoot"));
    first.action = Some(Box::new(disable));
    let mut inactive = script("InitiallyInactive");
    inactive.is_active = false;
    let mut own = Local::new(vec![first, script("LaterRoot"), inactive]);
    let peer = Local::new(vec![script("Peer")]);
    assert_eq!(
        own.team.read().unwrap().get_id(),
        peer.team.read().unwrap().get_id()
    );
    own.update();
    assert!(
        !own.engine
            .read()
            .unwrap()
            .as_ref()
            .unwrap()
            .find_script_clone_by_name("LaterRoot")
            .unwrap()
            .is_active(),
        "actual default DisableScript changed original root"
    );
    assert_eq!(
        *own.visits.0.lock().unwrap(),
        ["DisableRoot", "LaterRoot", "InitiallyInactive"]
    );
    assert!(peer.visits.0.lock().unwrap().is_empty());
    assert!(peer.team.read().unwrap().should_attempt_generic_script(0));
    assert!(!own.team.read().unwrap().should_attempt_generic_script(2));
}
#[test]
fn actual_generic_conditions_ignore_root_difficulty_flags() {
    let mut gated = script("NoDifficulties");
    gated.easy = false;
    gated.normal = false;
    gated.hard = false;
    let mut own = Local::new(vec![gated]);
    own.update();
    assert_eq!(*own.visits.0.lock().unwrap(), ["NoDifficulties"]);
}
#[test]
fn actual_generic_conditions_ignore_root_future_frame_without_mutating_it() {
    let mut gated = script("FutureFrame");
    gated.frame_to_evaluate_at = u32::MAX;
    let mut own = Local::new(vec![gated]);
    // Script list admission initializes root runtime fields. Set the actual
    // generic runtime's saved deadline directly to exercise its gate, not an
    // initialization shortcut or synthetic evaluator.
    let mut runtime = own
        .prototype
        .take_or_load_generic_script_runtime(0, &own.engine)
        .unwrap();
    runtime.frame_to_evaluate_at = u32::MAX;
    drop(runtime);
    own.update();
    assert_eq!(*own.visits.0.lock().unwrap(), ["FutureFrame"]);
    let runtime = own
        .prototype
        .take_or_load_generic_script_runtime(0, &own.engine)
        .unwrap();
    assert_eq!(runtime.frame_to_evaluate_at, u32::MAX);
    drop(runtime);
}
#[test]
fn actual_generic_repeatable_delay_does_not_write_root_periodic_deadline() {
    let mut repeated = script("RepeatDelay");
    repeated.is_one_shot = false;
    repeated.delay_evaluation_seconds = 7;
    let mut own = Local::new(vec![repeated]);
    let mut before = own
        .prototype
        .take_or_load_generic_script_runtime(0, &own.engine)
        .unwrap();
    // Isolate periodic scheduling from randomized root admission offsets.
    before.frame_to_evaluate_at = 0;
    let deadline = before.frame_to_evaluate_at;
    drop(before);
    own.update();
    own.update();
    assert_eq!(
        *own.visits.0.lock().unwrap(),
        ["RepeatDelay", "RepeatDelay"]
    );
    assert!(own.team.read().unwrap().should_attempt_generic_script(0));
    let runtime = own
        .prototype
        .take_or_load_generic_script_runtime(0, &own.engine)
        .unwrap();
    assert_eq!(runtime.frame_to_evaluate_at, deadline);
    assert_eq!(runtime.delay_evaluation_seconds, 7);
    drop(runtime);
}
