//! Player special-power and upgrade event consumption on the borrowed engine.
//! CPP ScriptConditions1443–1540 resolves player/source before querying the
//! event list with remove=true. Missing source/player cannot consume an edge.

use super::*;
use crate::scripting::engine::ScriptOwnerQuery;
use generals_sim::scripting::events::PlayerEventKind;

impl ScriptConditionEvaluator<'_> {
    /// Legacy worlds that supply no owned event-slot query use their existing roster.
    pub(super) fn eval_standalone_player_event(
        &self,
        condition: &Condition,
        driver: &dyn ScriptExecutionDriver,
    ) -> Result<Option<ScriptConditionResult>, ScriptError> {
        let Some((kind, from_named)) = PlayerEventKind::condition(condition.get_condition_type())
        else {
            return Ok(None);
        };
        let player_name = self.get_condition_string_param(condition, 0)?;
        let event_name = self.get_condition_string_param(condition, 1)?;
        let source_object = if from_named {
            // Player compact-slot admission is independent of named-object
            // ownership. Main supplies its cache even while that older player
            // adapter is selected; a missing own source never polls Core names.
            let raw_name = condition
                .get_parameter(2)
                .map(|parameter| parameter.get_string())
                .ok_or_else(|| ScriptError::ParameterNotFound("Parameter 2 not found".into()))?;
            match driver
                .named_object_id(raw_name, self.context.borrowed_engine().script_object_id())
            {
                ScriptOwnerQuery::Present(source) => source,
                ScriptOwnerQuery::Missing => return Ok(Some(ScriptConditionResult::False)),
                ScriptOwnerQuery::Unavailable => {
                    let source_name = self.get_condition_string_param(condition, 2)?;
                    let Some(source) = leftover_named_source_id(&self.host_query(), &source_name) else {
                        return Ok(Some(ScriptConditionResult::False));
                    };
                    source
                }
            }
        } else {
            INVALID_ID
        };
        let player_index = {
            let __player_list_arc = player_list();
            let Ok(players) = __player_list_arc.read() else {
                return Ok(Some(ScriptConditionResult::False));
            };
            let Some(player) = players.find_player_by_name(&player_name) else {
                return Ok(Some(ScriptConditionResult::False));
            };
            let Ok(player) = player.read() else {
                return Ok(Some(ScriptConditionResult::False));
            };
            player.get_player_index() as usize
        };
        let source = (source_object != INVALID_ID).then_some(source_object);
        let found = self.context.borrowed_engine().script_events().has_player_event(
            kind, player_index, &event_name, source, true,
        );
        Ok(Some(Self::bool_result(found)))
    }
}

/// C++ TheScriptEngine->getUnitNamed then Object::getID. Host IDs are not leftover crate Objects.
fn leftover_named_source_id(q: &crate::scripting::HostScriptQuery, unit_name: &str) -> Option<u32> {
    let tracker_id = get_named_object_tracker()
        .get_object_id(unit_name)
        .ok()
        .flatten();
    if crate::object::registry::OBJECT_REGISTRY.is_empty() {
        if let Some(obj) = q.host_script_query_object(unit_name) {
            return Some(obj.id);
        }
        if let Some(id) = tracker_id {
            if q.host_script_query_object_by_id(id).is_some() {
                return Some(id);
            }
        }
        return q.host_script_named_unit_id(unit_name).or(tracker_id);
    }
    let id = tracker_id?;
    TheGameLogic::find_object_by_id(id).map(|_| id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::engine::ScriptEngine;

    struct EventOwner {
        selected: ScriptOwnerQuery<usize>,
        source: ObjectID,
        player_calls: Vec<(String, u32, Option<String>)>,
        source_calls: RefCell<Vec<(String, Option<ObjectID>)>>,
    }

    impl ScriptExecutionDriver for EventOwner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn script_event_player_slot(
            &mut self,
            player: &str,
            mask: u32,
            current: Option<&str>,
        ) -> ScriptOwnerQuery<usize> {
            self.player_calls.push((player.into(), mask, current.map(str::to_owned)));
            if mask != 0 && !mask.is_power_of_two() {
                return ScriptOwnerQuery::Missing;
            }
            self.selected
        }

        fn named_object_id(
            &self,
            unit: &str,
            object: Option<ObjectID>,
        ) -> ScriptOwnerQuery<ObjectID> {
            self.source_calls.borrow_mut().push((unit.into(), object));
            ScriptOwnerQuery::Present(self.source)
        }
    }

    fn owner(source_object: ObjectID) -> EventOwner {
        EventOwner {
            selected: ScriptOwnerQuery::Present(1),
            source: source_object,
            player_calls: Vec::new(),
            source_calls: RefCell::new(Vec::new()),
        }
    }

    fn condition(
        kind: ConditionType,
        player: &str,
        event: &str,
        source: Option<&str>,
    ) -> Condition {
        let mut condition = Condition::new(kind);
        let event_type = if matches!(
            kind,
            ConditionType::PlayerBuiltUpgrade | ConditionType::PlayerBuiltUpgradeFromNamed
        ) {
            ParameterType::Upgrade
        } else {
            ParameterType::SpecialPower
        };
        for (parameter_type, value) in [(ParameterType::Side, player), (event_type, event)] {
            condition
                .add_parameter(Parameter::with_string(parameter_type, value.into()))
                .unwrap();
        }
        if let Some(source) = source {
            condition
                .add_parameter(Parameter::with_string(ParameterType::Unit, source.into()))
                .unwrap();
        }
        condition
    }

    fn notify(engine: &ScriptEngine, kind: ConditionType, event: &str, source: ObjectID) {
        let (kind, _) = PlayerEventKind::condition(kind).unwrap();
        engine.script_events().record_player_event(kind, 1, event, source);
    }

    #[test]
    fn all_eight_owned_event_conditions_consume_one_matching_edge() {
        for kind in [
            ConditionType::PlayerTriggeredSpecialPower,
            ConditionType::PlayerMidwaySpecialPower,
            ConditionType::PlayerCompletedSpecialPower,
            ConditionType::PlayerBuiltUpgrade,
            ConditionType::PlayerTriggeredSpecialPowerFromNamed,
            ConditionType::PlayerMidwaySpecialPowerFromNamed,
            ConditionType::PlayerCompletedSpecialPowerFromNamed,
            ConditionType::PlayerBuiltUpgradeFromNamed,
        ] {
            let engine = ScriptEngine::new_in_published_world().unwrap();
            let (_, from_named) = PlayerEventKind::condition(kind).unwrap();
            let source = if from_named { 42 } else { INVALID_ID };
            let mut driver = owner(source);
            let mut condition = condition(
                kind,
                "Owned",
                "SharedEvent",
                from_named.then_some("NamedSource"),
            );
            notify(&engine, kind, "SharedEvent", 42);
            let state = RefCell::new(ScriptContext::at_frame(0));
            let mut evaluator = ScriptConditionEvaluator::new(&engine, &state);
            assert_eq!(
                evaluator
                    .evaluate_condition_with_driver(&mut condition, &mut driver)
                    .unwrap(),
                ScriptConditionResult::True
            );
            assert_eq!(
                evaluator
                    .evaluate_condition_with_driver(&mut condition, &mut driver)
                    .unwrap(),
                ScriptConditionResult::False
            );
        }
    }

    #[test]
    fn duplicate_events_remove_first_match_only_and_preserve_other_sources() {
        let engine = ScriptEngine::new_in_published_world().unwrap();
        for source in [43, 42, 42] {
            engine.notify_of_triggered_special_power(1, "Power", source);
        }
        let mut driver = owner(42);
        let mut condition = condition(
            ConditionType::PlayerTriggeredSpecialPowerFromNamed,
            "Owned",
            "Power",
            Some("Named"),
        );
        let state = RefCell::new(ScriptContext::at_frame(0));
        let mut evaluator = ScriptConditionEvaluator::new(&engine, &state);
        for expected in [
            ScriptConditionResult::True,
            ScriptConditionResult::True,
            ScriptConditionResult::False,
        ] {
            assert_eq!(
                evaluator
                    .evaluate_condition_with_driver(&mut condition, &mut driver)
                    .unwrap(),
                expected
            );
        }
        assert!(engine.is_special_power_triggered(1, "Power", false, 43));
        assert!(engine.is_special_power_triggered(1, "Power", true, INVALID_ID));
        assert!(!engine.is_special_power_triggered(1, "Power", false, INVALID_ID));
    }

    #[test]
    fn authoritative_missing_does_not_consume_or_fall_back() {
        let engine = ScriptEngine::new_in_published_world().unwrap();
        engine.notify_of_completed_upgrade(1, "Upgrade", 42);
        let mut driver = owner(42);
        driver.selected = ScriptOwnerQuery::Missing;
        let mut condition = condition(
            ConditionType::PlayerBuiltUpgradeFromNamed,
            "MissingPlayer",
            "Upgrade",
            Some("MissingSource"),
        );
        let state = RefCell::new(ScriptContext::at_frame(0));
        let mut evaluator = ScriptConditionEvaluator::new(&engine, &state);
        assert_eq!(
            evaluator
                .evaluate_condition_with_driver(&mut condition, &mut driver)
                .unwrap(),
            ScriptConditionResult::False
        );
        assert!(engine.is_upgrade_complete(1, "Upgrade", false, 42));
        assert_eq!(condition.get_parameter(0).unwrap().get_int() as u32, 0xffff_0000);
        driver.selected = owner(42).selected;
        assert_eq!(
            evaluator
                .evaluate_condition_with_driver(&mut condition, &mut driver)
                .unwrap(),
            ScriptConditionResult::False
        );
        condition.parameters[0] = Some(Parameter::with_string(ParameterType::Side, "Owned".into()));
        assert_eq!(
            evaluator
                .evaluate_condition_with_driver(&mut condition, &mut driver)
                .unwrap(),
            ScriptConditionResult::True
        );
    }

    #[test]
    fn borrowed_engine_event_and_raw_tokens_win_over_foreign_lexical_scope() {
        let mut own = ScriptEngine::new_in_published_world().unwrap();
        own.set_external_eval_context(Some("OwnedCurrent".into()), None);
        let mut foreign = ScriptEngine::new_in_published_world().unwrap();
        foreign.set_external_eval_context(Some("ForeignCurrent".into()), None);
        own.notify_of_completed_special_power(1, LOCAL_PLAYER, 42);
        foreign.notify_of_completed_special_power(1, LOCAL_PLAYER, 42);
        let mut driver = owner(42);
        let mut condition = condition(
            ConditionType::PlayerCompletedSpecialPowerFromNamed,
            THIS_PLAYER,
            LOCAL_PLAYER,
            Some(THIS_OBJECT),
        );
        let state = RefCell::new(ScriptContext::at_frame(13));
        let mut evaluator = ScriptConditionEvaluator::new(&own, &state);
        foreign.with_active(|| {
            assert_eq!(
                evaluator
                    .evaluate_condition_with_driver(&mut condition, &mut driver)
                    .unwrap(),
                ScriptConditionResult::True
            );
        });
        assert_eq!(
            driver.player_calls,
            [(THIS_PLAYER.into(), 0, Some("OwnedCurrent".into()))]
        );
        assert_eq!(*driver.source_calls.borrow(), [(THIS_OBJECT.into(), None)]);
        assert!(!own.is_special_power_complete(1, LOCAL_PLAYER, false, 42));
        assert!(foreign.is_special_power_complete(1, LOCAL_PLAYER, false, 42));
    }

    #[test]
    fn default_driver_leaves_player_event_owner_selection_unavailable() {
        assert_eq!(
            CanonicalScriptExecutionDriver.script_event_player_slot("Unadmitted", 0, None),
            ScriptOwnerQuery::Unavailable
        );
    }

    #[test]
    fn missing_required_event_parameter_is_an_error_even_for_missing_owner() {
        let engine = ScriptEngine::new_in_published_world().unwrap();
        let mut driver = owner(42);
        driver.selected = ScriptOwnerQuery::Missing;
        let mut condition = Condition::new(ConditionType::PlayerTriggeredSpecialPower);
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::Side,
                "Missing".into(),
            ))
            .unwrap();
        let state = RefCell::new(ScriptContext::at_frame(0));
        let mut evaluator = ScriptConditionEvaluator::new(&engine, &state);
        assert!(matches!(
            evaluator.evaluate_condition_with_driver(&mut condition, &mut driver),
            Err(ScriptError::ParameterNotFound(_))
        ));
        assert!(driver.player_calls.is_empty());
        assert!(driver.source_calls.borrow().is_empty());
    }
}
