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
    for (case, truncate_tail) in [("Truncated", true), ("Trailing", false)] {
        let mut corrupt = decode_versioned_archive(&archive);
        if truncate_tail {
            corrupt.snapshot.lifecycle_tail.pop();
        } else {
            corrupt.snapshot.lifecycle_tail.push(0);
        }
        std::fs::write(
            saves.get_save_path(case),
            encode_versioned_archive(&corrupt),
        )
        .unwrap();
        let err = saves.load_game_snapshot(case).err().expect(case);
        assert!(
            matches!(err, SaveLoadError::Corrupted(_)),
            "{case}: corruption must fail closed: {err}"
        );
        assert_eq!(peer.host_object_id_by_script_name("Peer"), Some(donor));
    }

    let candidate = saves.load_game_snapshot("script_candidate").unwrap();
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
