use super::*;
use gamelogic::common::ObjectShroudStatus;

#[test]
fn new_world_shroud_does_not_inherit_process_match_state() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "new_world_shroud_does_not_inherit_process_match_state",
        || {
            // C++ PartitionManager.cpp:2515 starts with no cells/modules.
            // Pending reveals and previously seen objects belong to a match,
            // rather than the content definitions copied by world creation.
            let process = gamelogic::system::shroud_manager::get_shroud_manager();
            {
                let mut shroud = process.lock().unwrap();
                shroud.init_shroud_grid(240.0, 240.0);
                shroud.mark_host_object_seen(0, 1);
                shroud.set_host_object_shroud_status(0, 1, ObjectShroudStatus::Fogged);
            }
            let process_before = process.lock().unwrap().snapshot_state();
            let services = gamelogic::system::engine_stores::new_world_services();
            assert_eq!(
                services.shroud().lock().unwrap().snapshot_state(),
                Default::default(),
                "service construction must not copy a process match's grid"
            );
            let mut a = {
                // Construction needs no access to another match's visibility,
                // even when that match's shroud is already exclusively held.
                let _foreign_shroud = process.lock().unwrap();
                GameLogic::new()
            };
            {
                let shroud = a.world_services.shroud().lock().unwrap();
                assert_eq!(shroud.snapshot_state(), Default::default());
                assert_eq!(shroud.get_host_object_shroud_status(0, 1), None);
                assert!(!shroud.host_object_ever_seen(0, 1));
            }
            a.world_services
                .shroud()
                .lock()
                .unwrap()
                .reveal_map_for_player_permanently(0)
                .unwrap();
            let a_before = a.world_services.shroud().lock().unwrap().snapshot_state();
            let mut candidate = GameLogic::new();
            assert_eq!(
                candidate
                    .world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .snapshot_state(),
                Default::default(),
                "a detached load candidate cannot inherit process or A reveals"
            );
            let builder = crate::save_load::snapshot::SnapshotBuilder::new();
            let saved = builder.create_world_snapshot(&a).unwrap();
            let mut invalid: crate::save_load::snapshot::WorldSnapshot =
                serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
            invalid
                .shroud
                .pending_permanent_reveal_players
                .push(u32::MAX);
            assert!(
                builder
                    .restore_from_snapshot(&invalid, &mut candidate)
                    .is_err()
            );
            assert_eq!(
                candidate
                    .world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .snapshot_state(),
                Default::default(),
                "failed admission preserves the candidate's owned shroud"
            );
            candidate.reset();
            drop(candidate);
            assert_eq!(
                a.world_services.shroud().lock().unwrap().snapshot_state(),
                a_before
            );
            let mut restored = GameLogic::new();
            builder
                .restore_from_snapshot(&saved, &mut restored)
                .unwrap();
            assert_eq!(
                restored
                    .world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .snapshot_state(),
                a_before
            );
            a.reset();
            assert_eq!(
                restored
                    .world_services
                    .shroud()
                    .lock()
                    .unwrap()
                    .snapshot_state(),
                a_before
            );
            assert_eq!(process.lock().unwrap().snapshot_state(), process_before);
        },
    );
}

fn world_with_enemy() -> (GameLogic, ObjectId) {
    let mut logic = GameLogic::new();
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[crate::fixture_content::FixtureObject::new("ShroudOwnerUnit").kind_of("INFANTRY")],
    );
    logic.start_new_game(GameMode::Skirmish).unwrap();
    logic.clear_all_players();
    logic.add_player(Player::new(0, Team::USA, "Local", true));
    logic.add_player(Player::new(1, Team::China, "Enemy", false));
    let id = logic
        .create_object("ShroudOwnerUnit", Team::China, Vec3::ZERO)
        .unwrap();
    (logic, id)
}

#[test]
fn shroud_presentation_uses_driving_world_with_reused_object_ids() {
    let (mut left, left_id) = world_with_enemy();
    let (mut right, right_id) = world_with_enemy();
    assert_eq!(left_id, right_id);
    {
        let mut shroud = left.world_services.shroud().lock().unwrap();
        shroud.mark_host_vision_ready();
        shroud.mark_host_object_explored(0, left_id.0);
        shroud.set_host_object_shroud_status(0, left_id.0, ObjectShroudStatus::Fogged);
    }
    {
        let mut shroud = right.world_services.shroud().lock().unwrap();
        shroud.mark_host_vision_ready();
        shroud.mark_host_object_seen(0, right_id.0);
        shroud.set_host_object_shroud_status(0, right_id.0, ObjectShroudStatus::Clear);
    }
    let left_frame = PresentationFrame::try_build_from_logic(&mut left, 0).unwrap();
    let right_frame = PresentationFrame::try_build_from_logic(&mut right, 0).unwrap();
    assert_eq!(
        left_frame.fow_for_object(left_id),
        Some(crate::fow_rendering::ObjectVisibility::from_shroud_flags(
            false, true
        ))
    );
    assert_eq!(
        right_frame.fow_for_object(right_id),
        Some(crate::fow_rendering::ObjectVisibility::from_shroud_flags(
            true, true
        ))
    );
    right.reset();
    let after = PresentationFrame::try_build_from_logic(&mut left, 0).unwrap();
    assert_eq!(
        after.fow_for_object(left_id),
        left_frame.fow_for_object(left_id)
    );
}

#[test]
fn shroud_snapshot_captures_driving_world_after_another_world_starts() {
    let (left, _) = world_with_enemy();
    let (right, _) = world_with_enemy();
    left.world_services
        .shroud()
        .lock()
        .unwrap()
        .init_shroud_grid(400.0, 400.0);
    right
        .world_services
        .shroud()
        .lock()
        .unwrap()
        .init_shroud_grid(800.0, 800.0);
    let expected = left
        .world_services
        .shroud()
        .lock()
        .unwrap()
        .snapshot_state();
    let snapshot = crate::save_load::snapshot::SnapshotBuilder::new()
        .create_world_snapshot(&left)
        .unwrap();
    assert_eq!(
        snapshot.shroud.grid.as_ref().map(|g| (g.width, g.height)),
        expected.grid.as_ref().map(|g| (g.width, g.height))
    );
    assert!(
        snapshot.shroud == expected,
        "capture must preserve this world's raw counters and queues"
    );
}

#[test]
fn shroud_restore_replaces_only_the_driving_world_counters() {
    let (mut left, _) = world_with_enemy();
    let (right, _) = world_with_enemy();
    {
        let mut manager = left.world_services.shroud().lock().unwrap();
        manager.init_shroud_grid(400.0, 400.0);
        manager.do_shroud_reveal(&glam::Vec3::new(80.0, 80.0, 0.0), 100.0, 1);
    }
    right
        .world_services
        .shroud()
        .lock()
        .unwrap()
        .init_shroud_grid(800.0, 800.0);
    let right_before = right
        .world_services
        .shroud()
        .lock()
        .unwrap()
        .snapshot_state();
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&left).unwrap();
    left.world_services
        .shroud()
        .lock()
        .unwrap()
        .reset_for_new_game();
    builder.restore_from_snapshot(&snapshot, &mut left).unwrap();
    assert!(
        left.world_services
            .shroud()
            .lock()
            .unwrap()
            .snapshot_state()
            == snapshot.shroud
    );
    assert!(
        right
            .world_services
            .shroud()
            .lock()
            .unwrap()
            .snapshot_state()
            == right_before
    );
}

#[test]
fn same_tick_presentations_share_fog_until_receiving_cells_or_levels_change() {
    use crate::fow_rendering::{FOWRenderingBridge, ProjectedShroudSnapshot};
    use glam::Vec3 as Coord3D;
    use std::sync::Arc;

    let (mut logic, id) = world_with_enemy();
    let (mut peer, peer_id) = world_with_enemy();
    assert_eq!(id, peer_id);
    let origin = [40.0, 80.0];
    let center = Coord3D::new(160.0, 220.0, 0.0);
    for world in [&logic, &peer] {
        let mut shroud = world.world_services.shroud().lock().unwrap();
        shroud.init_shroud_grid_at_origin(origin, 480.0, 320.0);
        shroud.mark_host_vision_ready();
        shroud.mark_host_object_seen(0, id.0);
    }

    let publish = |world: &mut GameLogic| {
        let frame = PresentationFrame::try_build_from_logic(world, 0).unwrap();
        let shroud = world.world_services.shroud().lock().unwrap();
        let expected = FOWRenderingBridge::snapshot_terrain_grid(Some(&shroud), 0, false);
        assert_eq!(frame.fow_grid, expected);
        assert_eq!(
            frame.projected_shroud,
            ProjectedShroudSnapshot::from_grid(&expected, frame.projected_shroud.metadata),
        );
        frame
    };
    let initial = publish(&mut logic);
    let before_reveal = {
        let mut shroud = logic.world_services.shroud().lock().unwrap();
        let before = shroud.snapshot_state();
        shroud.do_shroud_reveal(&center, 45.0, 1);
        before
    };
    let revealed = publish(&mut logic);
    assert_eq!(initial.frame, revealed.frame);
    assert_ne!(initial.fow_grid.cells, revealed.fow_grid.cells);
    assert_ne!(initial.projected_shroud.texels, revealed.projected_shroud.texels);

    // Adding another looker and revealing for another player preserve viewer 0's bytes.
    {
        let mut shroud = logic.world_services.shroud().lock().unwrap();
        shroud.do_shroud_reveal(&center, 45.0, 1);
        shroud.do_shroud_reveal(&center, 45.0, 2);
    }
    let repeated = publish(&mut logic);
    assert!(Arc::ptr_eq(&revealed.fow_grid.cells, &repeated.fow_grid.cells));
    assert!(Arc::ptr_eq(&revealed.projected_shroud.texels, &repeated.projected_shroud.texels));
    let peer_frame = publish(&mut peer);
    assert_eq!(peer_frame.fow_grid, initial.fow_grid);
    assert!(!Arc::ptr_eq(&peer_frame.fow_grid.cells, &initial.fow_grid.cells));

    logic.session_view.border_shroud_level = Some(191);
    let bordered = publish(&mut logic);
    assert!(Arc::ptr_eq(&revealed.fow_grid.cells, &bordered.fow_grid.cells));
    assert_eq!(bordered.projected_shroud.texel_at(0, 0), Some(191));
    assert_ne!(bordered.projected_shroud.texels, revealed.projected_shroud.texels);

    // Two overlapping lookers leave a cell clear until the last is removed.
    logic.world_services.shroud().lock().unwrap().undo_shroud_reveal(&center, 45.0, 1);
    let still_revealed = publish(&mut logic);
    assert!(Arc::ptr_eq(&revealed.fow_grid.cells, &still_revealed.fow_grid.cells));
    logic.world_services.shroud().lock().unwrap().undo_shroud_reveal(&center, 45.0, 1);
    let explored = publish(&mut logic);
    assert_ne!(explored.fow_grid.cells, revealed.fow_grid.cells);
    assert!(explored.fow_grid.cells.contains(&crate::fow_rendering::PresentationFowGrid::CELL_EXPLORED));
    logic.world_services.shroud().lock().unwrap().do_shroud_cover(&center, 45.0, 1);
    assert_eq!(publish(&mut logic).fow_grid, initial.fow_grid);

    {
        let mut shroud = logic.world_services.shroud().lock().unwrap();
        shroud.replace_state_owned(&before_reveal, revealed.frame.0).unwrap();
        shroud.mark_host_vision_ready();
        shroud.mark_host_object_seen(0, id.0);
    }
    let restored = publish(&mut logic);
    assert_eq!(restored.fow_grid, initial.fow_grid);
    assert_eq!(restored.fow_grid.world_origin_xy, origin);
    assert_ne!(restored.fow_grid.cells, revealed.fow_grid.cells);
}
