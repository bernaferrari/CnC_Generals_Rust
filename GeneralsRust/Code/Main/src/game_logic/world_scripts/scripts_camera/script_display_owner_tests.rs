//! ScriptActions.cpp:2521–2539,3828: display effects belong to the driving game.
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::{ScriptDisplayRequest, ScriptExecutionDriver};
use gamelogic::scripting::engine::{ScriptEngine, get_script_engine};
use gamelogic::scripting::executor::ScriptContext;

fn display(kind: ScriptActionType, text: &str) -> Box<ScriptAction> {
    let mut action = ScriptAction::new(kind);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::TextString,
            text.into(),
        ))
        .unwrap();
    if kind == ScriptActionType::ShowMilitaryCaption {
        action
            .add_parameter(Parameter::with_int(ParameterType::Int, 4000))
            .unwrap();
    }
    Box::new(action)
}

fn authored_engine(text: &str, retained_hooks: Arc<MissionScriptHooks>) -> ScriptEngine {
    let mut actions = vec![
        display(ScriptActionType::DisplayText, text),
        display(ScriptActionType::DisplayCinematicText, text),
        display(ScriptActionType::ShowMilitaryCaption, text),
    ];
    let mut next = None;
    while let Some(mut action) = actions.pop() {
        action.next_action = next;
        next = Some(action);
    }
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "SameDisplayScript".into();
    script.is_one_shot = false;
    script.condition = Some(Box::new(condition));
    script.action = next;
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_action_handler(Some(Arc::new(MissionScriptActionHandler::new(
        retained_hooks,
    ))));
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine
}

fn walk(world: &mut GameLogic, engine: ScriptEngine) -> ScriptEngine {
    *get_script_engine().write().unwrap() = Some(engine);
    world.scripts_loaded = true;
    world.evaluate_and_execute_scripts(0.0);
    get_script_engine().write().unwrap().take().unwrap()
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn actual_main_display_actions_ignore_another_world_retained_handler() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "actual_main_display_actions_ignore_another_world_retained_handler",
        || {
            let mut first = GameLogic::new();
            let mut second = GameLogic::new();
            first.sim_time_seconds = 10.0;
            second.sim_time_seconds = 20.0;
            // Deliberately retain the other world's callbacks, as an engine
            // created before world replacement can. Execution's owner wins.
            let first_engine = authored_engine("first", second.mission_scripts.clone());
            let second_engine = authored_engine("second", first.mission_scripts.clone());
            assert!(first.new_script_messages.is_empty());
            assert!(second.new_script_messages.is_empty());

            let first_engine = walk(&mut first, first_engine);
            assert_eq!(first.new_script_messages, ["Transmission: first"]);
            assert_eq!(first.cinematic_text, Some(("first".into(), 10.0)));
            assert_eq!(first.cinematic_font.as_deref(), Some("Default"));
            assert_eq!(first.military_caption, Some(("first".into(), 14.0)));
            assert!(second.new_script_messages.is_empty());
            assert!(second.cinematic_text.is_none());
            assert!(second.military_caption.is_none());
            assert!(second.mission_scripts.drain_messages().is_empty());
            assert!(second.mission_scripts.drain_cinematic_text().is_empty());
            assert!(second.mission_scripts.drain_military_captions().is_empty());

            let second_engine = walk(&mut second, second_engine);
            assert_eq!(second.new_script_messages, ["Transmission: second"]);
            assert_eq!(second.cinematic_text, Some(("second".into(), 20.0)));
            assert_eq!(second.military_caption, Some(("second".into(), 24.0)));
            assert!(first.mission_scripts.drain_messages().is_empty());
            assert!(first.mission_scripts.drain_cinematic_text().is_empty());
            assert!(first.mission_scripts.drain_military_captions().is_empty());

            let _first_engine = walk(&mut first, first_engine);
            let _second_engine = walk(&mut second, second_engine);
            assert_eq!(
                first.new_script_messages,
                ["Transmission: first", "Transmission: first"]
            );
            assert_eq!(
                second.new_script_messages,
                ["Transmission: second", "Transmission: second"]
            );
            assert_eq!(first.mission_script_counter, 2);
            assert_eq!(second.mission_script_counter, 2);
        },
    );
}

struct DisplayObserver<'a> {
    world: &'a mut GameLogic,
    flushed: Vec<Vec<String>>,
}

impl ScriptExecutionDriver for DisplayObserver<'_> {
    fn display(
        &mut self,
        request: ScriptDisplayRequest<'_>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        HostScriptExecutionDriver::new(self.world).display(request)
    }

    fn after_action(&mut self) -> gamelogic::GameLogicResult<()> {
        HostScriptExecutionDriver::new(self.world).after_action()?;
        self.flushed.push(self.world.new_script_messages.clone());
        Ok(())
    }
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn actual_main_nested_display_flushes_before_the_next_instruction_without_a_handler() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "actual_main_nested_display_flushes_before_the_next_instruction_without_a_handler",
        || {
            let mut call = ScriptAction::new(ScriptActionType::CallSubroutine);
            call.add_parameter(Parameter::with_string(
                ParameterType::Script,
                "Inner".into(),
            ))
            .unwrap();
            call.next_action = Some(display(ScriptActionType::DisplayText, "after"));
            let mut before = display(ScriptActionType::DisplayText, "before");
            before.next_action = Some(Box::new(call));
            let mut condition = OrCondition::new();
            condition.set_first_and_condition(Some(Box::new(Condition::new(
                ConditionType::ConditionTrue,
            ))));
            let mut outer = Script::new();
            outer.script_name = "Outer".into();
            outer.is_one_shot = true;
            outer.condition = Some(Box::new(condition));
            outer.action = Some(before);
            let mut inner = Script::new();
            inner.script_name = "Inner".into();
            inner.is_subroutine = true;
            inner.condition = outer.condition.clone();
            let mut first_inner = display(ScriptActionType::DisplayText, "inner-a");
            first_inner.next_action = Some(display(ScriptActionType::DisplayText, "inner-b"));
            inner.action = Some(first_inner);
            let mut list = ScriptList::new();
            list.append_script(Box::new(outer));
            list.append_script(Box::new(inner));
            let mut engine = ScriptEngine::new().unwrap();
            assert!(engine.action_handler().is_none());
            engine
                .set_script_list_for_player(0, Some(Box::new(list)))
                .unwrap();
            let mut world = GameLogic::new();
            let mut context = ScriptContext::new();
            context.current_frame = world.frame;
            context.host_trigger_world = world.host_trigger_world.clone();
            let mut driver = DisplayObserver {
                world: &mut world,
                flushed: Vec::new(),
            };
            engine.update_with_driver(context, &mut driver).unwrap();
            // CPP ScriptEngine.cpp7609–7654: nested effects are visible before
            // returning to the outer action. The call itself also flushes.
            assert_eq!(
                driver.flushed,
                [
                    vec!["Transmission: before"],
                    vec!["Transmission: before", "Transmission: inner-a"],
                    vec![
                        "Transmission: before",
                        "Transmission: inner-a",
                        "Transmission: inner-b"
                    ],
                    vec![
                        "Transmission: before",
                        "Transmission: inner-a",
                        "Transmission: inner-b"
                    ],
                    vec![
                        "Transmission: before",
                        "Transmission: inner-a",
                        "Transmission: inner-b",
                        "Transmission: after"
                    ],
                ]
            );
            assert!(driver.world.mission_scripts.drain_messages().is_empty());
        },
    );
}
