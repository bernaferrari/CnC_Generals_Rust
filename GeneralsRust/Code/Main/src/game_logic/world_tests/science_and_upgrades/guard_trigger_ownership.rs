//! Actual guard scan regression for per-world polygon occupancy and reset.
use super::*;
use gamelogic::common::{AsciiString, ICoord3D};
use gamelogic::polygon_trigger::PolygonTrigger;

fn polygon(name: &str) -> PolygonTrigger {
    PolygonTrigger::new(
        7700,
        AsciiString::from(name),
        vec![
            ICoord3D::new(0, 0, 0),
            ICoord3D::new(100, 0, 0),
            ICoord3D::new(100, 100, 0),
            ICoord3D::new(0, 100, 0),
        ],
    )
}

fn populate(logic: &mut GameLogic, enemy_inside: bool) {
    let mut template = ThingTemplate::new("OwnedAreaGuard");
    template
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("OwnedAreaGuard".into(), template.clone());
    let mut guard = Object::new(template.clone(), ObjectId(77), Team::USA);
    guard.set_position(glam::Vec3::new(50.0, 0.0, 50.0));
    guard.weapon = Some(wave21_guard_weapon());
    logic.add_object(guard);
    let mut enemy = Object::new(template, ObjectId(88), Team::GLA);
    enemy.set_position(glam::Vec3::new(
        75.0,
        0.0,
        if enemy_inside { 50.0 } else { 150.0 },
    ));
    logic.add_object(enemy);
}

fn scan(logic: &GameLogic, area: &PolygonTrigger) -> Option<ObjectId> {
    logic.scan_guard_inner_target_for_test(
        ObjectId(77),
        Team::USA,
        glam::Vec3::new(50.0, 0.0, 50.0),
        200.0,
        false,
        false,
        false,
        Some(area),
    )
}

#[test]
fn same_named_guard_area_occupancy_is_owned_by_each_world() {
    let first_polygon = polygon("IsolatedGuardOccupancy");
    let second_polygon = polygon("IsolatedGuardOccupancy");
    let mut first = GameLogic::new();
    first.frame = 100;
    populate(&mut first, true);
    assert_eq!(scan(&first, &first_polygon), Some(ObjectId(88)));
    assert_eq!(first.frame_objects_changed_trigger_areas.get(), 100);
    first.frame += first.host_guard_enemy_scan_rate().max(1) + 1;
    assert_eq!(
        scan(&first, &first_polygon),
        None,
        "unchanged first-world occupants expire the CPP area scan window"
    );

    let mut second = GameLogic::new();
    assert_eq!(
        scan(&first, &first_polygon),
        None,
        "constructing another world must not invalidate this world's cache"
    );
    assert_eq!(first.frame_objects_changed_trigger_areas.get(), 100);
    second.frame = 20000;
    populate(&mut second, false);
    assert_eq!(scan(&second, &second_polygon), None);
    assert_eq!(
        second.frame_objects_changed_trigger_areas.get(),
        second.frame
    );
    assert_eq!(first.frame_objects_changed_trigger_areas.get(), 100);
    assert_eq!(
        scan(&first, &first_polygon),
        None,
        "same-name/same-ID other-world scan must not manufacture a local trigger change"
    );
    assert_eq!(first.frame_objects_changed_trigger_areas.get(), 100);
    second.reset();
    assert_eq!(
        scan(&first, &first_polygon),
        None,
        "resetting another world must not invalidate this world's occupancy cache"
    );
    assert_eq!(first.frame_objects_changed_trigger_areas.get(), 100);
}

#[test]
fn guard_area_stamp_and_occupancy_reset_before_same_id_recreation() {
    let area = polygon("ResetGuardOccupancy");
    let mut logic = GameLogic::new();
    logic.frame = 100;
    populate(&mut logic, true);
    assert_eq!(scan(&logic, &area), Some(ObjectId(88)));
    assert_eq!(logic.frame_objects_changed_trigger_areas.get(), 100);
    logic.reset();
    assert_eq!(
        logic.frame_objects_changed_trigger_areas.get(),
        0,
        "CPP GameLogic::reset clears the last trigger-change frame"
    );
    logic.frame = 20000;
    populate(&mut logic, true);
    assert_eq!(
        scan(&logic, &area),
        Some(ObjectId(88)),
        "first scan after local reset refreshes same-name/same-ID occupancy"
    );
    assert_eq!(logic.frame_objects_changed_trigger_areas.get(), logic.frame);
}

#[test]
fn snapshot_restore_invalidates_guard_occupancy_before_same_id_recreation() {
    let area = polygon("RestoredGuardOccupancy");
    let mut logic = GameLogic::new();
    logic.frame = 100;
    populate(&mut logic, true);
    assert_eq!(scan(&logic, &area), Some(ObjectId(88)));
    logic.frame = 20000;
    assert_eq!(scan(&logic, &area), None);
    assert_eq!(logic.frame_objects_changed_trigger_areas.get(), 100);

    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&logic)
        .expect("guard snapshot");
    builder
        .restore_from_snapshot(&snapshot, &mut logic)
        .expect("restore guard snapshot into used world");
    assert_eq!(logic.frame, 20000);
    assert!(logic.host_object(ObjectId(77)).is_some());
    assert!(logic.host_object(ObjectId(88)).is_some());
    // Host refresh remains lazy at the first scan; C++ Object admission
    // notifications set the restored-frame stamp eagerly (hq-df6h5).
    assert_eq!(
        scan(&logic, &area),
        Some(ObjectId(88)),
        "same restored names/IDs/positions must refresh this world's ephemeral cache"
    );
    assert_eq!(logic.frame_objects_changed_trigger_areas.get(), logic.frame);
}
