use super::*;
#[test]
fn sequence_capsule_without_team_calibrates() {
    let engine = ScriptEngine::new_in_published_world().unwrap();
    let records = vec![SequentialScriptSnapshot {
        script_name: "Sequence".into(),
        ..Default::default()
    }];
    engine.restore_sequential_scripts(&records);
    assert_eq!(engine.snapshot_sequential_scripts(), records);
}
#[test]
fn sequence_capsule_keeps_exact_reference_before_owner_binding() {
    let engine = ScriptEngine::new_in_published_world().unwrap();
    let records = vec![SequentialScriptSnapshot {
        team_id: 90,
        script_name: "Sequence".into(),
        ..Default::default()
    }];
    engine.restore_sequential_scripts(&records);
    assert_eq!(engine.snapshot_sequential_scripts(), records);
}

#[test]
fn sequential_xfer_resolves_program_in_the_receiving_interpreter() {
    use crate::system::engine_stores::{new_world_services, with_world_services};
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    fn install(engine: &ScriptEngine) {
        let mut body = Script::new();
        body.script_name = "ResumeAfterSave".into();
        let mut list = ScriptList::new();
        list.append_script(Box::new(body));
        engine
            .set_script_list_for_player(1, Some(Box::new(list)))
            .unwrap();
    }

    let source_world = new_world_services();
    let mut source = ScriptEngine::new_with_services(&source_world).unwrap();
    // Load installs the default fade when the saved interpreter has no fade.
    source.new_map();
    install(&source);
    source
        .named_objects
        .register_named_object("SourceObject".into(), 17)
        .unwrap();
    let expected = [SequentialScriptSnapshot {
        object_id: 17,
        script_name: "ResumeAfterSave".into(),
        current_instruction: 2,
        times_to_loop: 1,
        frames_to_wait: 5,
        dont_advance_instruction: true,
        ..Default::default()
    }];
    source.restore_sequential_scripts(&expected);
    let mut bytes = Vec::new();
    source
        .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
        .unwrap();

    let receiving_world = new_world_services();
    let peer_world = new_world_services();
    let object = with_world_services(&receiving_world, || {
        Arc::new(RwLock::new(crate::object::Object::new_test(17, 100.0)))
    });
    receiving_world
        .object_registry()
        .register_object(17, &object);
    peer_world
        .scripts()
        .named_objects()
        .register_named_object("PeerObject".into(), 17)
        .unwrap();
    let mut receiver = ScriptEngine::new_with_services(&receiving_world).unwrap();
    install(&receiver);
    with_world_services(&peer_world, || {
        receiver
            .xfer(&mut XferLoad::new(Cursor::new(&bytes), 1))
            .unwrap();
    });
    assert_eq!(receiver.snapshot_sequential_scripts(), expected);
    assert_eq!(
        receiver
            .named_objects
            .get_object_id("SourceObject")
            .unwrap(),
        Some(17)
    );
    assert_eq!(
        peer_world
            .scripts()
            .named_objects()
            .get_object_id("PeerObject")
            .unwrap(),
        Some(17)
    );
    let mut resaved = Vec::new();
    receiver
        .xfer(&mut XferSave::new(Cursor::new(&mut resaved), 1))
        .unwrap();
    assert_eq!(bytes, resaved);
}
