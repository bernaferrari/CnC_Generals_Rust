//! Player special-power and upgrade event consumption on the borrowed engine.
//! CPP ScriptConditions1443–1540 resolves player/source before querying the
//! event list with remove=true. Missing source/player cannot consume an edge.

use super::*;
use crate::scripting::engine::{ScriptOwnerQuery, ScriptPlayerEventSource};

#[derive(Clone, Copy)]
enum PlayerEventKind {
    Triggered,
    Midway,
    Completed,
    Upgrade,
}

impl PlayerEventKind {
    fn condition(condition_type: ConditionType) -> Option<(Self, bool)> {
        use ConditionType::*;
        Some(match condition_type {
            PlayerTriggeredSpecialPower => (Self::Triggered, false),
            PlayerMidwaySpecialPower => (Self::Midway, false),
            PlayerCompletedSpecialPower => (Self::Completed, false),
            PlayerBuiltUpgrade => (Self::Upgrade, false),
            PlayerTriggeredSpecialPowerFromNamed => (Self::Triggered, true),
            PlayerMidwaySpecialPowerFromNamed => (Self::Midway, true),
            PlayerCompletedSpecialPowerFromNamed => (Self::Completed, true),
            PlayerBuiltUpgradeFromNamed => (Self::Upgrade, true),
            _ => return None,
        })
    }

    fn consume(
        self,
        engine: &crate::scripting::engine::ScriptEngine,
        selected: ScriptPlayerEventSource,
        name: &str,
    ) -> bool {
        let ScriptPlayerEventSource {
            player_index,
            source_object,
        } = selected;
        match self {
            Self::Triggered => {
                engine.is_special_power_triggered(player_index, name, true, source_object)
            }
            Self::Midway => engine.is_special_power_midway(player_index, name, true, source_object),
            Self::Completed => {
                engine.is_special_power_complete(player_index, name, true, source_object)
            }
            Self::Upgrade => engine.is_upgrade_complete(player_index, name, true, source_object),
        }
    }
}

impl ScriptConditionEvaluator<'_> {
    /// None means another condition family. This family resolves one owner,
    /// then consumes through its selected or retained standalone path.
    pub(super) fn eval_player_event(
        &self,
        condition: &Condition,
        driver: &dyn ScriptExecutionDriver,
    ) -> Result<Option<ScriptConditionResult>, ScriptError> {
        let Some((kind, from_named)) = PlayerEventKind::condition(condition.get_condition_type())
        else {
            return Ok(None);
        };
        let raw = |index| {
            condition
                .get_parameter(index)
                .map(|parameter| parameter.get_string())
                .ok_or_else(|| {
                    ScriptError::ParameterNotFound(format!("Parameter {index} not found"))
                })
        };
        let player = raw(0)?;
        let name = raw(1)?;
        let source = if from_named { Some(raw(2)?) } else { None };
        let engine = self.context.borrowed_engine();
        let current_player = engine.get_current_player_name();
        let this_object = engine.script_object_id();
        let selected = match driver.player_event_source(
            player,
            source,
            current_player.as_deref(),
            this_object,
        ) {
            ScriptOwnerQuery::Unavailable => {
                return self
                    .eval_standalone_player_event(condition, kind, from_named)
                    .map(Some);
            }
            ScriptOwnerQuery::Missing => return Ok(Some(ScriptConditionResult::False)),
            ScriptOwnerQuery::Present(selected) => selected,
        };
        // Event names are also literal in C++; do not rewrite token-looking
        // special-power/upgrade names through global player resolution.
        let found = kind.consume(engine, selected, name);
        Ok(Some(Self::bool_result(found)))
    }

    /// Retained standalone adapter: preserve token resolution, named-source
    /// lookup before player admission, and the existing player-index mapping.
    fn eval_standalone_player_event(
        &self,
        condition: &Condition,
        kind: PlayerEventKind,
        from_named: bool,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let event_name = self.get_condition_string_param(condition, 1)?;
        let source_object = if from_named {
            let source_name = self.get_condition_string_param(condition, 2)?;
            let Some(source) = leftover_named_source_id(&source_name) else {
                return Ok(ScriptConditionResult::False);
            };
            source
        } else {
            INVALID_ID
        };
        let player_index = {
            let Ok(players) = player_list().read() else {
                return Ok(ScriptConditionResult::False);
            };
            let Some(player) = players.find_player_by_name(&player_name) else {
                return Ok(ScriptConditionResult::False);
            };
            let Ok(player) = player.read() else {
                return Ok(ScriptConditionResult::False);
            };
            player.get_player_index() as usize
        };
        let selected = ScriptPlayerEventSource {
            player_index,
            source_object,
        };
        Ok(Self::bool_result(kind.consume(
            self.context.borrowed_engine(),
            selected,
            &event_name,
        )))
    }
}

/// C++ TheScriptEngine->getUnitNamed then Object::getID. Host IDs are not leftover crate Objects.
fn leftover_named_source_id(unit_name: &str) -> Option<u32> {
    let tracker_id = get_named_object_tracker()
        .get_object_id(unit_name)
        .ok()
        .flatten();
    if crate::object::registry::OBJECT_REGISTRY.is_empty() {
        if let Some(obj) = crate::scripting::host_script_query_object(unit_name) {
            return Some(obj.id);
        }
        if let Some(id) = tracker_id {
            if crate::scripting::host_script_query_object_by_id(id).is_some() {
                return Some(id);
            }
        }
        return crate::scripting::host_script_named_unit_id(unit_name).or(tracker_id);
    }
    let id = tracker_id?;
    TheGameLogic::find_object_by_id(id).map(|_| id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::engine::ScriptEngine;

    struct EventOwner {
        selected: ScriptOwnerQuery<ScriptPlayerEventSource>,
        calls: RefCell<Vec<(String, Option<String>, Option<String>, Option<ObjectID>)>>,
    }

    impl ScriptExecutionDriver for EventOwner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn player_event_source(
            &self,
            player: &str,
            source: Option<&str>,
            current: Option<&str>,
            object: Option<ObjectID>,
        ) -> ScriptOwnerQuery<ScriptPlayerEventSource> {
            self.calls.borrow_mut().push((
                player.into(),
                source.map(str::to_owned),
                current.map(str::to_owned),
                object,
            ));
            self.selected
        }
    }

    fn owner(source_object: ObjectID) -> EventOwner {
        EventOwner {
            selected: ScriptOwnerQuery::Present(ScriptPlayerEventSource {
                player_index: 1,
                source_object,
            }),
            calls: RefCell::new(Vec::new()),
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
        match PlayerEventKind::condition(kind).unwrap().0 {
            PlayerEventKind::Triggered => {
                engine.notify_of_triggered_special_power(1, event, source)
            }
            PlayerEventKind::Midway => engine.notify_of_midway_special_power(1, event, source),
            PlayerEventKind::Completed => {
                engine.notify_of_completed_special_power(1, event, source)
            }
            PlayerEventKind::Upgrade => engine.notify_of_completed_upgrade(1, event, source),
        }
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
            let engine = ScriptEngine::new().unwrap();
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
        let engine = ScriptEngine::new().unwrap();
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
        let engine = ScriptEngine::new().unwrap();
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
        driver.selected = owner(42).selected;
        assert_eq!(
            evaluator
                .evaluate_condition_with_driver(&mut condition, &mut driver)
                .unwrap(),
            ScriptConditionResult::True
        );
    }

    #[test]
    fn borrowed_engine_event_and_raw_tokens_win_over_foreign_lexical_scope() {
        let mut own = ScriptEngine::new().unwrap();
        own.set_external_eval_context(Some("OwnedCurrent".into()), None);
        let mut foreign = ScriptEngine::new().unwrap();
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
            *driver.calls.borrow(),
            vec![(
                THIS_PLAYER.into(),
                Some(THIS_OBJECT.into()),
                Some("OwnedCurrent".into()),
                None
            )]
        );
        assert!(!own.is_special_power_complete(1, LOCAL_PLAYER, false, 42));
        assert!(foreign.is_special_power_complete(1, LOCAL_PLAYER, false, 42));
    }

    #[test]
    fn default_driver_leaves_player_event_owner_selection_unavailable() {
        assert_eq!(
            CanonicalScriptExecutionDriver.player_event_source("Unadmitted", None, None, None),
            ScriptOwnerQuery::Unavailable
        );
    }

    #[test]
    fn missing_required_event_parameter_is_an_error_even_for_missing_owner() {
        let engine = ScriptEngine::new().unwrap();
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
        assert!(driver.calls.borrow().is_empty());
    }
}
