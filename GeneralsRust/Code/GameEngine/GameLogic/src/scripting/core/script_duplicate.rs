// Source-style duplication of an admitted authored Script tree.
// CPP constructors obtain parameter shape from templates; this operation retains
// the already-admitted Rust shape. It does not normalize malformed/changed templates.
impl Script {
    /// CPP Scripts.cpp:971-1000: duplicate one definition, not its linked list
    /// or accumulated execution state. Parameter assignment retains its caches.
    pub(crate) fn duplicate_definition(&self) -> Self {
        fn condition(source: &Condition) -> Box<Condition> {
            Box::new(Condition {
                condition_type: source.condition_type,
                num_parms: source.num_parms,
                parameters: source.parameters.clone(),
                next_and_condition: source.next_and_condition.as_deref().map(condition),
                has_warnings: false,
                custom_data: 0,
                custom_frame: 0,
            })
        }
        fn or_condition(source: &OrCondition) -> Box<OrCondition> {
            Box::new(OrCondition {
                first_and: source.first_and.as_deref().map(condition),
                next_or: source.next_or.as_deref().map(or_condition),
            })
        }
        // Keep Rust's initialized editor-warning default. The original typed
        // action constructor leaves that editor-only field unspecified.
        fn action(source: &ScriptAction) -> Box<ScriptAction> {
            Box::new(ScriptAction {
                action_type: source.action_type,
                num_parms: source.num_parms,
                parameters: source.parameters.clone(),
                next_action: source.next_action.as_deref().map(action),
                has_warnings: false,
            })
        }
        Self {
            script_name: self.script_name.clone(),
            comment: self.comment.clone(),
            condition_comment: self.condition_comment.clone(),
            action_comment: self.action_comment.clone(),
            delay_evaluation_seconds: self.delay_evaluation_seconds,
            is_active: self.is_active,
            is_one_shot: self.is_one_shot,
            is_subroutine: self.is_subroutine,
            easy: self.easy,
            normal: self.normal,
            hard: self.hard,
            condition: self.condition.as_deref().map(or_condition),
            action: self.action.as_deref().map(action),
            action_false: self.action_false.as_deref().map(action),
            ..Self::new()
        }
    }
}
