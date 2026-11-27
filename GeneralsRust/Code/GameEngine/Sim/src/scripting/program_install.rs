//! Initialize installed script programs in C++ ScriptList order.
//! ScriptEngine.cpp:5430–5460 and 6865–6920.

use super::program::{ParameterType, Script, ScriptList};
use crate::time::LOGIC_HZ;

pub fn initialize_script_list(
    list: &mut ScriptList,
    mut random_int: impl FnMut(i32, i32) -> i32,
    mut team_is_singleton: impl FnMut(&str) -> Option<bool>,
) {
    let mut initialize_chain = |mut scripts: Option<&mut Script>| {
        while let Some(script) = scripts {
            script.frame_to_evaluate_at = if script.delay_evaluation_seconds > 0 {
                random_int(0, 2 * LOGIC_HZ as i32).max(0) as u32
            } else {
                0
            };
            assign_condition_team(script, &mut team_is_singleton);
            scripts = script.next_script.as_deref_mut();
        }
    };
    initialize_chain(list.first_script.as_deref_mut());
    let mut groups = list.first_group.as_deref_mut();
    while let Some(group) = groups {
        initialize_chain(group.first_script.as_deref_mut());
        groups = group.next_group.as_deref_mut();
    }
}

fn assign_condition_team(
    script: &mut Script,
    team_is_singleton: &mut impl FnMut(&str) -> Option<bool>,
) {
    let mut last_singleton = None;
    let mut first_multiple = None;
    let branches = std::iter::successors(script.condition.as_deref(), |node| {
        node.get_next_or_condition()
    });
    for branch in branches {
        let conditions = std::iter::successors(branch.first_and.as_deref(), |node| node.get_next());
        for condition in conditions {
            for index in 0..condition.get_num_parameters() {
                let Some(parameter) = condition.get_parameter(index) else {
                    continue;
                };
                if parameter.get_parameter_type() != ParameterType::Team {
                    continue;
                }
                let name = parameter.get_string().trim();
                if name.is_empty() {
                    continue;
                }
                match team_is_singleton(name) {
                    Some(true) => last_singleton = Some(name),
                    Some(false) => match first_multiple {
                        None => first_multiple = Some(name),
                        Some(previous) if previous != name => log::warn!(
                            "Script '{}' contains multiple non-singleton team conditions: '{}' and '{}'",
                            script.script_name,
                            previous,
                            name
                        ),
                        Some(_) => {}
                    },
                    None => {}
                }
            }
        }
    }
    if let Some(name) = first_multiple.or(last_singleton) {
        script.condition_team_name = name.into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::program::{
        Condition, ConditionType, OrCondition, Parameter, ScriptGroup,
    };

    fn program(delay: i32, names: &[&str]) -> Box<Script> {
        let mut script = Script::new();
        script.delay_evaluation_seconds = delay;
        script.frame_to_evaluate_at = 99;
        script.condition_team_name = "saved hint".into();
        let mut condition = Condition::new(ConditionType::ConditionTrue);
        for name in names {
            condition
                .add_parameter(Parameter::with_string(ParameterType::Team, (*name).into()))
                .unwrap();
        }
        let mut branch = OrCondition::new();
        branch.first_and = Some(Box::new(condition));
        script.condition = Some(Box::new(branch));
        Box::new(script)
    }

    #[test]
    fn initialization_draws_for_delayed_roots_before_groups_and_prefers_the_first_multi_team() {
        let mut list = ScriptList::new();
        list.append_script(program(1, &["Singleton", "Many", "AlsoMany"]));
        list.append_script(program(0, &["Missing"]));
        let mut group = ScriptGroup::new(0);
        group.is_group_active = false;
        group.first_script = Some(program(2, &["Singleton", "LastSingleton"]));
        list.first_group = Some(Box::new(group));

        let mut draws = [17, 42].into_iter();
        initialize_script_list(
            &mut list,
            |lo, hi| {
                assert_eq!((lo, hi), (0, 60));
                draws.next().unwrap()
            },
            |name| match name {
                "Singleton" | "LastSingleton" => Some(true),
                "Many" | "AlsoMany" => Some(false),
                _ => None,
            },
        );
        assert_eq!(draws.next(), None);
        let root = list.get_script().unwrap();
        let next = root.get_next().unwrap();
        let grouped = list.get_script_group().unwrap().get_script().unwrap();
        assert_eq!(
            (root.frame_to_evaluate_at, root.condition_team_name.as_str()),
            (17, "Many")
        );
        assert_eq!(
            (next.frame_to_evaluate_at, next.condition_team_name.as_str()),
            (0, "saved hint")
        );
        assert_eq!(
            (
                grouped.frame_to_evaluate_at,
                grouped.condition_team_name.as_str()
            ),
            (42, "LastSingleton")
        );
    }
}
