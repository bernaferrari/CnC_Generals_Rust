use super::*;
use gamelogic::common::ObjectShroudStatus;

fn world_with_enemy() -> (GameLogic, ObjectId) {
    let mut logic = GameLogic::new();
    logic.start_new_game(GameMode::Skirmish);
    logic.clear_all_players();
    logic.add_player(Player::new(0, Team::USA, "Local", true));
    logic.add_player(Player::new(1, Team::China, "Enemy", false));
    let mut template = ThingTemplate::new("ShroudOwnerUnit");
    template.add_kind_of(KindOf::Infantry);
    logic.templates.insert(template.name.clone(), template);
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
        let mut shroud = left.engine_stores.shroud().lock().unwrap();
        shroud.mark_host_vision_ready();
        shroud.mark_host_object_explored(0, left_id.0);
        shroud.set_host_object_shroud_status(0, left_id.0, ObjectShroudStatus::Fogged);
    }
    {
        let mut shroud = right.engine_stores.shroud().lock().unwrap();
        shroud.mark_host_vision_ready();
        shroud.mark_host_object_seen(0, right_id.0);
        shroud.set_host_object_shroud_status(0, right_id.0, ObjectShroudStatus::Clear);
    }
    let left_frame = PresentationFrame::build_from_logic(&mut left, 0);
    let right_frame = PresentationFrame::build_from_logic(&mut right, 0);
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
    let after = PresentationFrame::build_from_logic(&mut left, 0);
    assert_eq!(
        after.fow_for_object(left_id),
        left_frame.fow_for_object(left_id)
    );
}

#[test]
fn shroud_snapshot_captures_driving_world_after_another_world_starts() {
    let (left, _) = world_with_enemy();
    let (right, _) = world_with_enemy();
    left.engine_stores
        .shroud()
        .lock()
        .unwrap()
        .init_shroud_grid(400.0, 400.0);
    right
        .engine_stores
        .shroud()
        .lock()
        .unwrap()
        .init_shroud_grid(800.0, 800.0);
    let expected = left.engine_stores.shroud().lock().unwrap().snapshot_state();
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
        let mut manager = left.engine_stores.shroud().lock().unwrap();
        manager.init_shroud_grid(400.0, 400.0);
        manager.do_shroud_reveal(&gamelogic::common::Coord3D::new(80.0, 80.0, 0.0), 100.0, 1);
    }
    right
        .engine_stores
        .shroud()
        .lock()
        .unwrap()
        .init_shroud_grid(800.0, 800.0);
    let right_before = right
        .engine_stores
        .shroud()
        .lock()
        .unwrap()
        .snapshot_state();
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&left).unwrap();
    left.engine_stores
        .shroud()
        .lock()
        .unwrap()
        .reset_for_new_game();
    builder.restore_from_snapshot(&snapshot, &mut left).unwrap();
    assert!(left.engine_stores.shroud().lock().unwrap().snapshot_state() == snapshot.shroud);
    assert!(
        right
            .engine_stores
            .shroud()
            .lock()
            .unwrap()
            .snapshot_state()
            == right_before
    );
}
