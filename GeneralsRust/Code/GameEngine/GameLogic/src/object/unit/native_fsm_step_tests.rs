//! Exercise the ordinary update of the factory's installed native machine.
use super::*;
use crate::ai::states::AIStateType;
use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};

fn captured_base_wire(actual: &FactoryRuntime, ai: &mut dyn AIUpdateInterface) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    assert!(
        ai.xfer_ai_update_state(&mut XferSave::new(&mut bytes, 1))
            .unwrap()
    );
    // Use the production decoder only to inspect captured bytes. This reader
    // is never installed, driven, or substituted for the actual cached AI.
    let reader = crate::object::object_factory::factory_ai::prepare_unit_ai(
        &actual.owner,
        &crate::common::DefaultThingTemplate::new("StepWireReader".into()),
        actual.id,
    );
    let mut reader = reader.lock().unwrap();
    assert!(
        reader
            .xfer_ai_update_state(&mut XferLoad::new(Cursor::new(bytes.into_inner()), 1))
            .unwrap()
    );
    let mut base_bytes = Cursor::new(Vec::new());
    reader
        .ai_state_machine
        .as_mut()
        .unwrap()
        .base
        .xfer(&mut XferSave::new(&mut base_bytes, 1))
        .unwrap();
    base_bytes.into_inner()
}

fn sleep_deadline(bytes: &[u8]) -> u32 {
    // Common/StateMachine.cpp:803-815: version byte, sleepTill, default ID,
    // current ID. No fixture-only getter or alternative machine is needed.
    assert_eq!(bytes[0], 1);
    assert_eq!(
        u32::from_le_bytes(bytes[9..13].try_into().unwrap()),
        AIStateType::Idle as u32
    );
    u32::from_le_bytes(bytes[1..5].try_into().unwrap())
}

#[test]
fn factory_idle_update_runs_body_then_preserves_sleeping_wire() {
    if !child(concat!(
        module_path!(),
        "::factory_idle_update_runs_body_then_preserves_sleeping_wire"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    let _frame = RestoreAmbientFrame::set(17);
    let actual = FactoryRuntime::new();
    let mut ai = actual.ai.lock().unwrap();
    ai.execute_command(&AiCommandParams::new(
        AiCommandType::Idle,
        CommandSourceType::FromPlayer,
    ))
    .unwrap();
    assert_eq!(ai.get_current_state_id(), Some(AIStateType::Idle as u32));
    assert_eq!(sleep_deadline(&captured_base_wire(&actual, &mut *ai)), 0);
    ai.update().unwrap();
    let after = captured_base_wire(&actual, &mut *ai);
    // AIStates.cpp:1304/1371/1446: initial idle offset 0..60, then sleep
    // 60+offset logic frames. A skipped native body would leave sleepTill=0.
    assert!((77..=137).contains(&sleep_deadline(&after)));
    ai.update().unwrap();
    assert_eq!(captured_base_wire(&actual, &mut *ai), after);
    assert_eq!(ai.get_current_command(), Some(AiCommandType::Idle));
    assert!(super::super::registry::get_unit_arc(actual.id).is_none());
    assert!(Arc::ptr_eq(
        &actual.ai,
        &actual
            .owner
            .read()
            .unwrap()
            .get_ai_update_interface()
            .unwrap()
    ));
}
