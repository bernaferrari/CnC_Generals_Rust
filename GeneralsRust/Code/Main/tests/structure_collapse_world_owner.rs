use generals_main::game_logic::{GameLogic, ObjectAudio, Player, SessionAudio, Team};
use glam::Vec3;

fn world(delay_ms: u32, initial_fx: &str) -> (GameLogic, generals_main::game_logic::ObjectId) {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([(
            "structure-collapse-owner.ini".to_string(),
            format!(
                "Object SharedCollapseProbe\n  KindOf = STRUCTURE IMMOBILE\n  Body = ActiveBody BodyTag\n    MaxHealth = 100\n    InitialHealth = 100\n  End\n  Behavior = StructureCollapseUpdate ModuleTag_Sink\n    MinBurstDelay = {delay_ms}\n    MaxBurstDelay = {delay_ms}\n    FXList = INITIAL {initial_fx}\n  End\nEnd\n"
            ),
        )])
        .expect("admit receiving Object INI into this world");
    world.add_player(Player::new(1, Team::USA, "Collapse Owner", false));
    let id = world
        .create_object_for_player("SharedCollapseProbe", 1, Vec3::ZERO)
        .expect("create authored structure");
    (world, id)
}

#[test]
fn same_id_worlds_begin_collapse_from_their_own_admitted_ini() {
    let (mut first, first_id) = world(2200, "FX_FirstCollapseInitial");
    let (mut second, second_id) = world(4400, "FX_SecondCollapseInitial");
    assert_eq!(first_id, second_id);

    let mut first_audio = SessionAudio::default();
    let mut second_audio = SessionAudio::default();
    assert!(
        first
            .host_object_mut(first_id)
            .unwrap()
            .begin_structure_collapse(0, &mut ObjectAudio::queue(&mut first_audio),)
    );
    assert!(
        second
            .host_object_mut(second_id)
            .unwrap()
            .begin_structure_collapse(0, &mut ObjectAudio::queue(&mut second_audio),)
    );

    let first_data = first
        .host_object(first_id)
        .unwrap()
        .structure_collapse_data
        .as_ref()
        .unwrap();
    let second_data = second
        .host_object(second_id)
        .unwrap()
        .structure_collapse_data
        .as_ref()
        .unwrap();
    assert_eq!(first_data.min_burst_delay, 66);
    assert_eq!(
        first_data.fx_initial.as_deref(),
        Some("FX_FirstCollapseInitial")
    );
    assert_eq!(second_data.min_burst_delay, 132);
    assert_eq!(
        second_data.fx_initial.as_deref(),
        Some("FX_SecondCollapseInitial")
    );

    first
        .host_object_mut(first_id)
        .unwrap()
        .tick_structure_collapse(1);
    second
        .host_object_mut(second_id)
        .unwrap()
        .tick_structure_collapse(1);
    assert_eq!(
        first
            .host_object(first_id)
            .unwrap()
            .structure_collapse_data
            .as_ref()
            .unwrap()
            .min_burst_delay,
        66
    );
    assert_eq!(
        second
            .host_object(second_id)
            .unwrap()
            .structure_collapse_data
            .as_ref()
            .unwrap()
            .min_burst_delay,
        132
    );
}

#[test]
fn real_death_and_snapshot_continuation_keep_each_worlds_collapse_definition() {
    let (mut first, first_id) = world(2200, "FX_FirstCollapseInitial");
    let (mut peer, peer_id) = world(4400, "FX_SecondCollapseInitial");
    assert_eq!(first_id, peer_id);

    assert_eq!(first.apply_host_damage(first_id, 200.0), (true, false));
    assert_eq!(peer.apply_host_damage(peer_id, 200.0), (true, false));
    for (owner, id, delay, fx) in [
        (&first, first_id, 66, "FX_FirstCollapseInitial"),
        (&peer, peer_id, 132, "FX_SecondCollapseInitial"),
    ] {
        let object = owner.host_object(id).expect("dead collapse body retained");
        assert_eq!(object.health.current, 0.0);
        assert!(object.status.destroyed);
        assert!(object.status.on_die_started);
        let collapse = object
            .structure_collapse_data
            .as_ref()
            .expect("real lethal damage begins the authored collapse update");
        assert!(collapse.is_active());
        assert_eq!(collapse.min_burst_delay, delay);
        assert_eq!(collapse.fx_initial.as_deref(), Some(fx));
        assert!(collapse.initial_played);
    }

    // Advance beyond the shared waiting period so the snapshot contains live
    // sink motion and a world-specific next burst frame.
    for _ in 0..25 {
        first.update();
        peer.update();
    }
    let first_collapse = first
        .host_object(first_id)
        .unwrap()
        .structure_collapse_data
        .as_ref()
        .unwrap();
    let peer_collapse = peer
        .host_object(peer_id)
        .unwrap()
        .structure_collapse_data
        .as_ref()
        .unwrap();
    assert!(first_collapse.start_burst_played);
    assert!(peer_collapse.start_burst_played);
    assert_eq!(
        first_collapse.burst_frame,
        first_collapse.collapse_start_frame + first_collapse.min_burst_delay
    );
    assert_eq!(
        peer_collapse.burst_frame,
        peer_collapse.collapse_start_frame + peer_collapse.min_burst_delay
    );

    let directory = tempfile::tempdir().expect("temporary save directory");
    let mut saves = generals_main::save_load::SaveFileManager::with_save_directory(directory.path());
    saves.init().expect("initialize save manager");
    saves
        .quick_save(&first)
        .expect("save active receiving collapse continuation");
    let decoded = saves
        .load_game_snapshot("quicksave")
        .expect("decode the written save archive");
    let (mut restored, restored_id) = world(2200, "FX_FirstCollapseInitial");
    assert_eq!(restored_id, first_id);
    saves
        .restore_game_snapshot(&decoded.snapshot, &mut restored)
        .expect("restore into the same admitted receiving collapse catalog");
    let restored_object = restored.host_object(restored_id).unwrap();
    let restored_collapse = restored_object.structure_collapse_data.as_ref().unwrap();
    assert_eq!(restored_object.health.current, 0.0);
    assert!(restored_object.status.destroyed);
    assert_eq!(restored_collapse.min_burst_delay, 66);
    assert_eq!(restored_collapse.burst_frame, first_collapse.burst_frame);
    assert_eq!(
        restored_collapse.fx_initial.as_deref(),
        Some("FX_FirstCollapseInitial")
    );
    assert!(restored_collapse.start_burst_played);

    // Restoring the first owner's continuation does not replace the peer's
    // same-name object definition or delay.
    assert_eq!(
        peer.host_object(peer_id)
            .unwrap()
            .structure_collapse_data
            .as_ref()
            .unwrap()
            .min_burst_delay,
        132
    );
    for _ in 0..800 {
        first.update();
        peer.update();
        restored.update();
    }
    for (owner, id, delay, fx) in [
        (&first, first_id, 66, "FX_FirstCollapseInitial"),
        (&peer, peer_id, 132, "FX_SecondCollapseInitial"),
        (&restored, restored_id, 66, "FX_FirstCollapseInitial"),
    ] {
        let object = owner
            .host_object(id)
            .expect("collapse body remains installed");
        let collapse = object.structure_collapse_data.as_ref().unwrap();
        assert_eq!(collapse.state as u8, 3, "collapse reaches DONE");
        assert_eq!(collapse.min_burst_delay, delay);
        assert_eq!(collapse.fx_initial.as_deref(), Some(fx));
        assert!(collapse.initial_played);
        assert!(collapse.final_played);
        assert_eq!(object.health.current, 0.0);
    }
}
