//! Actual Main cursor queries, including colliding IDs and unrelated prototypes.
use super::condition_cursor_control_tests::admit;
use super::named_command_test_support::world;
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::engine::{ScriptExecutionDriver, ScriptOwnerQuery};
#[test]
fn live_cursor_is_owned_and_wrong_prototype_or_missing_ids_are_final() {
    let (mut first, member, _) = world();
    let (mut other, foreign, _) = world();
    admit(&first, "Copies", 10, Some(member));
    admit(&first, "Copies", 20, None);
    admit(&first, "OtherProto", 15, None);
    admit(&other, "Copies", 10, Some(foreign));
    assert_eq!(
        HostScriptExecutionDriver::new(&mut first).condition_team_after("Copies", None),
        ScriptOwnerQuery::Present(Some(20))
    );
    assert_eq!(
        HostScriptExecutionDriver::new(&mut first).condition_team_after("Copies", Some(20)),
        ScriptOwnerQuery::Present(Some(10))
    );
    assert_eq!(
        HostScriptExecutionDriver::new(&mut first).condition_team_after("Copies", Some(10)),
        ScriptOwnerQuery::Present(None)
    );
    assert_eq!(
        HostScriptExecutionDriver::new(&mut other).condition_team_after("Copies", None),
        ScriptOwnerQuery::Present(Some(10))
    );
    for id in [15, 999] {
        assert_eq!(
            HostScriptExecutionDriver::new(&mut first).condition_team_after("Copies", Some(id)),
            ScriptOwnerQuery::Missing
        );
    }
    assert_eq!(
        HostScriptExecutionDriver::new(&mut first).condition_team_after("Unknown", None),
        ScriptOwnerQuery::Missing
    );
    let mut factory = first.team_factory.lock().unwrap();
    assert_eq!(factory.condition_team_after("Copies", Some(15)), None);
    let deletion = factory.prepare_host_team_deletion(20).unwrap();
    factory.finalize_host_team_deletion(deletion);
    drop(factory);
    assert_eq!(
        HostScriptExecutionDriver::new(&mut first).condition_team_after("Copies", None),
        ScriptOwnerQuery::Present(Some(10))
    );
    assert_eq!(
        HostScriptExecutionDriver::new(&mut other).condition_team_after("Copies", None),
        ScriptOwnerQuery::Present(Some(10))
    );
}
