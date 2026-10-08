// Physical asset load evidence. This is a parse trace, not full gameplay parity.
mod retail_script_key_tests {
    use super::*;
    use gamelogic::scripting::engine::{ScriptEngine, get_script_engine};
    struct RestoreEngine(Option<ScriptEngine>);
    impl Drop for RestoreEngine {
        fn drop(&mut self) {
            *get_script_engine().write().unwrap() = self.0.take();
        }
    }
    fn trace_scripts(
        script: Option<&Script>,
        rows: &mut Vec<(
            String,
            Vec<ConditionType>,
            Vec<ScriptActionType>,
            Vec<ScriptActionType>,
        )>,
    ) {
        let mut cursor = script;
        while let Some(script) = cursor {
            let mut conditions = Vec::new();
            let mut or = script.condition.as_deref();
            while let Some(node) = or {
                let mut and = node.first_and.as_deref();
                while let Some(node) = and {
                    conditions.push(node.condition_type);
                    and = node.next_and_condition.as_deref();
                }
                or = node.next_or.as_deref();
            }
            let actions = |mut action: Option<&ScriptAction>| {
                let mut result = Vec::new();
                while let Some(node) = action {
                    result.push(node.action_type);
                    action = node.next_action.as_deref();
                }
                result
            };
            rows.push((
                script.script_name.clone(),
                conditions,
                actions(script.action.as_deref()),
                actions(script.action_false.as_deref()),
            ));
            cursor = script.next_script.as_deref();
        }
    }
    #[test]
    #[ignore = "requires untracked retail map; set GENERALS_SCRIPT_KEY_RETAIL_MAP and run explicitly"]
    fn physical_map_script_key_trace() {
        let path = std::env::var("GENERALS_SCRIPT_KEY_RETAIL_MAP").expect("retail path required");
        let engine = ScriptEngine::new().unwrap();
        let previous = {
            let handle = get_script_engine();
            let mut slot = handle.write().unwrap();
            std::mem::replace(&mut *slot, Some(engine))
        };
        let _restore = RestoreEngine(previous);
        let document = load_chunky_map(&path)
            .unwrap()
            .expect("physical map exists");
        let result = load_map_scripts_from_chunky(&document)
            .unwrap()
            .expect("script chunks exist");
        assert!(result.total_scripts > 0);
        let mut rows = Vec::new();
        for list in &result.script_lists {
            trace_scripts(list.first_script.as_deref(), &mut rows);
            let mut group = list.first_group.as_deref();
            while let Some(node) = group {
                trace_scripts(node.first_script.as_deref(), &mut rows);
                group = node.next_group.as_deref();
            }
        }
        assert_eq!(rows.len(), result.total_scripts);
        eprintln!(
            "retail_script_key_trace={}",
            serde_json::to_string(&rows).unwrap()
        );
    }
}
