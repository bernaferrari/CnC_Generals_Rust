//! Original DLINK advance observes callback mutations; no shared fixtures.
use super::*;
use crate::scripting::executor::ScriptContext;

fn guard() -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::TeamGuard);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            THIS_TEAM.into(),
        ))
        .unwrap();
    action
}
fn engine() -> ScriptEngine {
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.condition_team_name = "Copies".into();
    script.is_one_shot = true;
    script.condition = Some(Box::new(branch));
    script.action = Some(Box::new(guard()));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let mut engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine.set_external_eval_context(None, Some(777));
    engine
}
#[derive(Clone, Copy)]
enum Mutation {
    RemoveNext,
    AddHead,
    RemoveCurrent,
    NestedRemoveNext,
}
struct Owner<'a> {
    engine: &'a ScriptEngine,
    ids: Vec<TeamID>,
    seen: Vec<(Option<TeamID>, Option<TeamID>)>,
    mutation: Mutation,
    changed: bool,
}
impl ScriptExecutionDriver for Owner<'_> {
    fn team_instances(&self, name: &str) -> ScriptOwnerQuery<Vec<TeamID>> {
        if name == "Copies" {
            ScriptOwnerQuery::Present(self.ids.clone())
        } else {
            ScriptOwnerQuery::Missing
        }
    }
    fn team_name(&self, id: TeamID) -> ScriptOwnerQuery<String> {
        if self.ids.contains(&id) {
            ScriptOwnerQuery::Present("Copies".into())
        } else {
            ScriptOwnerQuery::Missing
        }
    }
    fn team_current_player(&self, id: TeamID) -> ScriptOwnerQuery<Option<String>> {
        if self.ids.contains(&id) {
            ScriptOwnerQuery::Present(None)
        } else {
            ScriptOwnerQuery::Missing
        }
    }
    fn team_guard(
        &mut self,
        _: &str,
        calling: Option<TeamID>,
        condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        self.seen.push((calling, condition));
        if !self.changed {
            self.changed = true;
            match self.mutation {
                Mutation::RemoveNext => self.ids.retain(|id| *id != 20),
                Mutation::AddHead => self.ids.insert(0, 40),
                Mutation::RemoveCurrent => self.ids.retain(|id| Some(*id) != condition),
                Mutation::NestedRemoveNext => {
                    let engine = self.engine;
                    engine.friend_execute_action_with_driver(
                        &guard(),
                        condition,
                        ScriptContext::at_frame(1),
                        self,
                    );
                    self.ids.retain(|id| *id != 20);
                    assert_eq!(engine.get_condition_team_id(), condition);
                    assert_eq!(engine.get_calling_team_id(), None);
                }
            }
        }
        Some(Ok(()))
    }
    fn after_action(&mut self) -> GameLogicResult<()> {
        Ok(())
    }
}
fn walk(mutation: Mutation) -> Vec<(Option<TeamID>, Option<TeamID>)> {
    let engine = engine();
    let mut owner = Owner {
        engine: &engine,
        ids: vec![30, 20, 10],
        seen: vec![],
        mutation,
        changed: false,
    };
    engine
        .update_with_driver(ScriptContext::at_frame(1), &mut owner)
        .unwrap();
    assert_eq!(engine.get_condition_team_id(), Some(777));
    owner.seen
}
#[test]
fn next_deleted_during_callback_is_not_evaluated() {
    assert_eq!(
        walk(Mutation::RemoveNext),
        vec![(None, Some(30)), (None, Some(10))]
    );
}
#[test]
fn prepended_head_during_callback_is_not_visited() {
    assert_eq!(
        walk(Mutation::AddHead),
        vec![(None, Some(30)), (None, Some(20)), (None, Some(10))]
    );
}
#[test]
fn missing_current_ends_safely_without_rebinding_to_next_sibling() {
    // The original iterator would dereference a freed current pointer. This
    // checks Rust safety, not a claim about defined C++ gameplay behavior.
    assert_eq!(walk(Mutation::RemoveCurrent), vec![(None, Some(30))]);
}
#[test]
fn nested_callback_restores_context_and_outer_walk_observes_new_next() {
    assert_eq!(
        walk(Mutation::NestedRemoveNext),
        vec![(None, Some(30)), (Some(30), Some(30)), (None, Some(10))]
    );
}
