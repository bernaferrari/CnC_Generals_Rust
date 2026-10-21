use super::*;

fn append_depth_chain(list: &mut ScriptList, prefix: &str, count: u32, flag: &str) {
    for depth in 0..count {
        let action = if depth + 1 == count {
            set_flag_action(flag)
        } else {
            call_subroutine_action(&format!("{prefix}{}", depth + 1))
        };
        list.append_script(Box::new(subroutine(&format!("{prefix}{depth}"), action)));
    }
}

fn engine_with_list(list: ScriptList) -> ScriptEngine {
    let mut engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine
}

struct PeerSubroutine<'a> {
    peer: &'a ScriptEngine,
    called: bool,
    found: Option<bool>,
}

impl ScriptExecutionDriver for PeerSubroutine<'_> {
    fn after_action(&mut self) -> GameLogicResult<()> {
        if !self.called {
            self.called = true;
            self.found = Some(self.peer.execute_subroutine_by_name("PeerEntry0")?);
        }
        Ok(())
    }
}

#[test]
fn subroutine_depth_is_isolated_between_engines() {
    // C++ callSubroutine executes immediately on its receiving ScriptEngine.
    // A second engine must not inherit the first engine's safety-limit budget.
    let mut outer_list = ScriptList::new();
    append_depth_chain(
        &mut outer_list,
        "Outer",
        MAX_SUBROUTINE_CALL_DEPTH,
        "outer_reached",
    );
    let outer = engine_with_list(outer_list);
    let mut peer_list = ScriptList::new();
    append_depth_chain(&mut peer_list, "PeerEntry", 1, "peer_reached");
    let peer = engine_with_list(peer_list);
    let mut driver = PeerSubroutine {
        peer: &peer,
        called: false,
        found: None,
    };

    assert!(
        outer
            .execute_subroutine_by_name_with_driver("Outer0", ScriptContext::new(), &mut driver)
            .unwrap()
    );
    assert!(
        outer
            .get_flag("outer_reached")
            .is_some_and(|flag| flag.value)
    );
    assert_eq!(
        driver.found,
        Some(true),
        "a peer engine must retain its own subroutine budget at the outer engine's limit"
    );
    assert!(peer.get_flag("peer_reached").is_some_and(|flag| flag.value));
}

#[test]
fn subroutine_depth_limit_is_retained_and_released_on_return() {
    let mut list = ScriptList::new();
    append_depth_chain(
        &mut list,
        "TooDeep",
        MAX_SUBROUTINE_CALL_DEPTH + 1,
        "must_not_run",
    );
    append_depth_chain(
        &mut list,
        "AfterLimit",
        MAX_SUBROUTINE_CALL_DEPTH,
        "after_limit",
    );
    let engine = engine_with_list(list);
    assert!(engine.execute_subroutine_by_name("TooDeep0").unwrap());
    assert!(engine.get_flag("must_not_run").is_none());
    assert!(engine.execute_subroutine_by_name("AfterLimit0").unwrap());
    assert!(
        engine
            .get_flag("after_limit")
            .is_some_and(|flag| flag.value)
    );
}

struct FailAfterAction {
    panic: bool,
}

impl ScriptExecutionDriver for FailAfterAction {
    fn after_action(&mut self) -> GameLogicResult<()> {
        assert!(!self.panic, "subroutine owner unwind fixture");
        Err(GameLogicError::Configuration(
            "subroutine owner error fixture".to_string(),
        ))
    }
}

#[test]
fn subroutine_depth_is_released_after_error_and_unwind() {
    for panic in [false, true] {
        let mut list = ScriptList::new();
        append_depth_chain(
            &mut list,
            "Interrupted",
            MAX_SUBROUTINE_CALL_DEPTH,
            "interrupted_reached",
        );
        append_depth_chain(
            &mut list,
            "Recovered",
            MAX_SUBROUTINE_CALL_DEPTH,
            "recovered_reached",
        );
        let engine = engine_with_list(list);
        let mut driver = FailAfterAction { panic };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            engine.execute_subroutine_by_name_with_driver(
                "Interrupted0",
                ScriptContext::new(),
                &mut driver,
            )
        }));
        if panic {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result,
                Ok(Err(GameLogicError::Configuration(message)))
                    if message == "subroutine owner error fixture"
            ));
        }
        assert!(
            engine
                .get_flag("interrupted_reached")
                .is_some_and(|flag| flag.value)
        );
        assert!(engine.execute_subroutine_by_name("Recovered0").unwrap());
        assert!(
            engine
                .get_flag("recovered_reached")
                .is_some_and(|flag| flag.value)
        );
    }
}
