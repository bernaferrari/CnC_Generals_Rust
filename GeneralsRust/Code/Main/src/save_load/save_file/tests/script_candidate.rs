use super::*;

fn receiving() -> GameLogic {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::USA, "Script owner", true));
    let mut template = ThingTemplate::new("ArchivedScriptActor");
    template.set_health(100.0);
    world.templates.insert(template.name.clone(), template);
    world
}

fn stage_scripts(world: &mut GameLogic) {
    world.scripts_loaded = true;
    crate::game_logic::evaluate_and_execute_scripts_for_test(world, 1.0 / 30.0);
}

fn actor(world: &mut GameLogic, name: &str) -> ObjectId {
    let id = world
        .create_object_for_player("ArchivedScriptActor", 1, Vec3::ZERO)
        .unwrap();
    assert!(world.set_host_object_name(id, name));
    id
}

fn replace_script_block(archive: &[u8], replacement: &[u8]) -> Vec<u8> {
    let mut cursor = Cursor::new(Vec::new());
    let mut xfer = CommonXferSave::new(&mut cursor, SAVE_FILE_VERSION);
    for (name, payload) in walk_named_chunks(archive).unwrap() {
        let payload = if name == CHUNK_SCRIPT_ENGINE {
            replacement
        } else {
            &payload
        };
        write_named_block(&mut xfer, &name, |xfer| {
            xfer.xfer_readonly(payload)
                .map_err(|err| SaveLoadError::Serialization(err.to_string()))
        })
        .unwrap();
    }
    write_ascii(&mut xfer, SAVE_FILE_EOF).unwrap();
    cursor.into_inner()
}

#[test]
fn script_archive_rejects_corruption_and_preserves_ordered_named_continuation() {
    let directory = tempfile::tempdir().unwrap();
    let mut saves = SaveFileManager::with_save_directory(directory.path());
    let mut control = receiving();
    let donor = actor(&mut control, "Donor");
    let recipient = actor(&mut control, "Recipient");
    stage_scripts(&mut control);
    assert!(control.transfer_script_object_name(donor, recipient));
    assert_eq!(
        control.host_object_id_by_script_name("Donor"),
        Some(recipient)
    );
    let mut peer = receiving();
    assert_eq!(actor(&mut peer, "Peer"), donor);
    stage_scripts(&mut peer);

    saves
        .save_game("script_candidate", &control, &fixture_save_info())
        .unwrap();
    let archive = std::fs::read(saves.get_save_path("script_candidate")).unwrap();
    let script = walk_named_chunks(&archive)
        .unwrap()
        .into_iter()
        .find(|(name, _)| name == CHUNK_SCRIPT_ENGINE)
        .unwrap()
        .1;
    let mut trailing = script.clone();
    trailing.push(0);
    let mut foreign = SnapshotBuilder::new()
        .create_world_snapshot(&control)
        .unwrap();
    foreign
        .persist_v18
        .script_engine_tail
        .as_mut()
        .unwrap()
        .named_objects[0]
        .0 = "Foreign".into();
    let mut cursor = Cursor::new(Vec::new());
    let mut xfer = CommonXferSave::new(&mut cursor, SAVE_FILE_VERSION);
    crate::save_load::snapshot::persist_v18::write_script_engine_block(
        &mut xfer,
        &foreign.persist_v18,
    )
    .unwrap();
    for (case, payload) in [
        ("Truncated", &script[..script.len() - 1]),
        ("Trailing", trailing.as_slice()),
        ("Conflicting", cursor.get_ref().as_slice()),
    ] {
        std::fs::write(
            saves.get_save_path(case),
            replace_script_block(&archive, payload),
        )
        .unwrap();
        let err = saves.load_game_snapshot(case).err().expect(case);
        assert!(err.to_string().contains("ScriptEngine"), "{case}: {err}");
        assert_eq!(peer.host_object_id_by_script_name("Peer"), Some(donor));
    }

    let candidate = saves.load_game_snapshot("script_candidate").unwrap();
    std::fs::write(
        saves.get_save_path("Placeholder"),
        replace_script_block(&archive, &[1]),
    )
    .unwrap();
    let historical = saves.load_game_snapshot("Placeholder").unwrap();
    assert_eq!(
        historical.snapshot.persist_v18.script_engine_tail,
        candidate.snapshot.persist_v18.script_engine_tail,
    );
    let mut restored = receiving();
    saves
        .restore_game_snapshot(&candidate.snapshot, &mut restored)
        .unwrap();
    for step in 0..4 {
        if step == 1 {
            for world in [&mut control, &mut restored] {
                world.destroy_object(recipient).unwrap();
                world.process_destroy_list();
            }
        }
        for world in [&mut control, &mut restored] {
            world.scripts_loaded = true;
            let previous_frame = world.getFrame();
            world.update();
            assert_eq!(world.getFrame(), previous_frame + 1);
            assert_eq!(
                world.host_object_id_by_script_name("Donor"),
                (step == 0).then_some(recipient)
            );
            assert_eq!(world.host_object_id_by_script_name("Recipient"), None);
            assert_eq!(world.host_object(donor).unwrap().name, "Donor");
        }
        assert_eq!(restored.getFrame(), control.getFrame());
        assert_eq!(peer.host_object_id_by_script_name("Peer"), Some(donor));
    }
}
