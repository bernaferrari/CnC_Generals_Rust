//! Ordinary Main pose writes and owner notifications; no fake queue insertion.
use super::*;
use crate::game_logic::GameLogic;
use gamelogic::common::{AsciiString, ICoord3D};
use gamelogic::polygon_trigger::PolygonTrigger;

fn trigger(id: i32, name: &str) -> PolygonTrigger {
    PolygonTrigger::new(
        id,
        AsciiString::from(name),
        vec![
            ICoord3D::new(100, 0, 0),
            ICoord3D::new(130, 0, 0),
            ICoord3D::new(130, 30, 0),
            ICoord3D::new(100, 30, 0),
        ],
    )
}

fn fixture(area: &PolygonTrigger) -> (GameLogic, ObjectId) {
    let mut parser = crate::assets::IniParser::new();
    assert_eq!(
        parser
            .parse_ini_content(
                r#"
Object PoseOwnerProbe
  Draw = W3DModelDraw ModuleTag_Draw
    DefaultConditionState
      Model = PoseOwnerProbe
    End
  End
  KindOf = VEHICLE SELECTABLE
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 100
  End
End
"#,
                "pose_owner.ini"
            )
            .unwrap(),
        1
    );
    let mut logic = GameLogic::new();
    assert_eq!(
        logic.seed_asset_definition_templates_from_snapshot(
            parser
                .get_all_definitions()
                .iter()
                .map(|(n, d)| (n.clone(), d.clone()))
        ),
        1
    );
    logic
        .host_trigger_world
        .lock()
        .unwrap()
        .set_trigger_areas(&[area.clone()]);
    logic
        .host_trigger_world
        .lock()
        .unwrap()
        .set_current_frame(37);
    let id = logic
        .create_object("PoseOwnerProbe", Team::USA, Vec3::new(90.0, 0.0, 10.0))
        .unwrap();
    (logic, id)
}

fn trigger_pose(logic: &GameLogic, id: ObjectId) -> (i32, i32) {
    let entries = logic.host_trigger_world.lock().unwrap().capture();
    let entry = entries.iter().find(|e| e.object_id == id.0).unwrap();
    (entry.i_x, entry.i_y)
}

fn pose_translation_keeps_physics_rotation_axes_case() {
    let mut template = ThingTemplate::new("PoseRotationProbe");
    template.add_kind_of(KindOf::Vehicle);
    let mut object = Object::new(template, ObjectId(17), Team::USA);
    object.apply_physics_ypr(0.0, 0.35, 0.25);
    let before = object.get_transform_matrix();
    assert_ne!(
        before.y_axis,
        glam::Mat4::IDENTITY.y_axis,
        "fixture has actual pitch/roll"
    );
    object.set_position(Vec3::new(7.0, 4.0, 11.0));
    let after = object.get_transform_matrix();
    assert_eq!(
        after.x_axis, before.x_axis,
        "C++ Thing::setPosition changes translation only"
    );
    assert_eq!(after.y_axis, before.y_axis);
    assert_eq!(after.z_axis, before.z_axis);
    assert_eq!(object.get_position(), Vec3::new(7.0, 4.0, 11.0));
}

fn pose_physics_translation_notifies_own_trigger_before_census_case() {
    let area_a = trigger(27101, "PoseAreaA");
    let area_b = trigger(27102, "PoseAreaB");
    let (mut a, aid) = fixture(&area_a);
    let (b, bid) = fixture(&area_b);
    assert_eq!(aid, bid, "same allocator-local identity in distinct owners");
    assert_eq!(trigger_pose(&a, aid), (90, 10));
    let position = {
        let (object, health_events) = a.host_object_and_health_events_mut(aid).unwrap();
        object.movement.velocity = Vec3::new(15.0, 0.0, 0.0);
        let _ = object.tick_physics_motion_step(0.0, health_events);
        object.get_position()
    };
    assert!(
        position.x > 100.0,
        "real physics translation reached the authored polygon"
    );
    assert_eq!(
        trigger_pose(&a, aid),
        (position.x as i32, position.z as i32),
        "physics pose transition must update its owning trigger state immediately"
    );
    assert!(
        a.host_trigger_world
            .lock()
            .unwrap()
            .did_enter(aid.0, &area_a, 37)
    );
    assert_eq!(
        trigger_pose(&b, bid),
        (90, 10),
        "another same-ID world is unaffected"
    );
    // Do not inject a script census here: that would mask the stale transition.
    let snapshot = crate::save_load::snapshot::SnapshotBuilder::new()
        .create_world_snapshot(&a)
        .unwrap();
    let saved = snapshot
        .object_triggers
        .iter()
        .find(|e| e.object_id == aid)
        .unwrap();
    assert_eq!((saved.i_x, saved.i_y), trigger_pose(&a, aid));
}

fn pose_setter_clone_and_current_snapshot_are_inert_controls_case() {
    let area = trigger(27103, "PoseControl");
    let (mut logic, id) = fixture(&area);
    logic
        .host_object_mut(id)
        .unwrap()
        .set_position(Vec3::new(110.0, 2.0, 10.0));
    let before = format!("{:?}", logic.host_trigger_world.lock().unwrap().capture());
    let object = logic.host_object(id).unwrap();
    let cloned = object.clone();
    assert_eq!(cloned.get_position(), object.get_position());
    assert_eq!(cloned.get_transform_matrix(), object.get_transform_matrix());
    let encoded = serde_json::to_vec(object).unwrap();
    let decoded: Object = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(decoded.get_position(), object.get_position());
    assert_eq!(
        format!("{:?}", logic.host_trigger_world.lock().unwrap().capture()),
        before
    );
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&logic).unwrap();
    assert_eq!(
        snapshot.objects.get(&id).unwrap().geometry.position,
        object.get_position()
    );
    assert_eq!(
        format!("{:?}", logic.host_trigger_world.lock().unwrap().capture()),
        before
    );
}

fn pose_default_public_update_keeps_transform_and_saved_geometry_coherent_case() {
    let area = trigger(27104, "PosePublicUpdate");
    let (mut logic, id) = fixture(&area);
    assert_eq!(
        *logic.gameworld_authority(),
        Default::default(),
        "ordinary Main is authoritative"
    );
    let _ = logic.update_with_dt(1.0 / 30.0);
    let object = logic.host_object(id).unwrap();
    assert_eq!(
        object.get_position(),
        object.get_transform_matrix().w_axis.truncate()
    );
    let snapshot = crate::save_load::snapshot::SnapshotBuilder::new()
        .create_world_snapshot(&logic)
        .unwrap();
    assert_eq!(
        snapshot.objects.get(&id).unwrap().geometry.position,
        object.get_position()
    );
}

#[test]
fn pose_translation_keeps_physics_rotation_axes() {
    isolated_at(
        module_path!(),
        "pose_translation_keeps_physics_rotation_axes",
        || {
            crate::gameworld_shadow::with_gameworld_authority(
                crate::game_logic::game_logic::gameworld_authority::GameWorldAuthority::DEFAULT_OFF,
                pose_translation_keeps_physics_rotation_axes_case,
            );
        },
    );
}

#[test]
fn pose_physics_translation_notifies_own_trigger_before_census() {
    isolated_at(
        module_path!(),
        "pose_physics_translation_notifies_own_trigger_before_census",
        || {
            crate::gameworld_shadow::with_gameworld_authority(
                crate::game_logic::game_logic::gameworld_authority::GameWorldAuthority::DEFAULT_OFF,
                pose_physics_translation_notifies_own_trigger_before_census_case,
            );
        },
    );
}

#[test]
fn pose_setter_clone_and_current_snapshot_are_inert_controls() {
    isolated_at(
        module_path!(),
        "pose_setter_clone_and_current_snapshot_are_inert_controls",
        || {
            crate::gameworld_shadow::with_gameworld_authority(
                crate::game_logic::game_logic::gameworld_authority::GameWorldAuthority::DEFAULT_OFF,
                pose_setter_clone_and_current_snapshot_are_inert_controls_case,
            );
        },
    );
}

#[test]
fn pose_default_public_update_keeps_transform_and_saved_geometry_coherent() {
    isolated_at(
        module_path!(),
        "pose_default_public_update_keeps_transform_and_saved_geometry_coherent",
        || {
            crate::gameworld_shadow::with_gameworld_authority(
                crate::game_logic::game_logic::gameworld_authority::GameWorldAuthority::DEFAULT_OFF,
                pose_default_public_update_keeps_transform_and_saved_geometry_coherent_case,
            );
        },
    );
}

#[cfg(not(target_arch = "wasm32"))]
fn isolated_at(module_path: &str, test: &str, run: impl FnOnce()) {
    use std::io::Read;
    use std::process::Stdio;

    const CHILD: &str = "GENERALS_OBJECT_POSE_OWNER_CHILD";
    let module = module_path.split_once("::").unwrap().1;
    let exact = format!("{module}::{test}");
    if std::env::var(CHILD).ok().as_deref() == Some(exact.as_str()) {
        run();
        return;
    }
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &exact, "--nocapture", "--test-threads=1"])
        .env(CHILD, &exact)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn exact pose owner regression");
    // Drain both pipes while the child runs; verbose failure output must not
    // fill a pipe and turn an assertion into a misleading watchdog timeout.
    let pipes: [Box<dyn Read + Send>; 2] = [
        Box::new(child.stdout.take().unwrap()),
        Box::new(child.stderr.take().unwrap()),
    ];
    let readers = pipes.map(|mut pipe| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            pipe.read_to_end(&mut bytes)
                .expect("drain pose owner child");
            bytes
        })
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll pose owner child") {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            timed_out = true;
            child.kill().expect("kill stalled pose owner child");
            break child.wait().expect("reap stalled pose owner child");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    let output = readers.map(|reader| {
        String::from_utf8_lossy(&reader.join().expect("join pose owner reader")).into_owned()
    });
    assert!(
        !timed_out,
        "pose owner child {exact} exceeded 30 seconds: {output:?}"
    );
    assert!(
        status.success(),
        "pose owner child {exact} failed: {status}: {output:?}"
    );
    assert!(
        output[0].contains("1 passed; 0 failed"),
        "exact child ran no regression: {exact}: {output:?}"
    );
}

#[cfg(target_arch = "wasm32")]
fn isolated_at(_: &str, _: &str, run: impl FnOnce()) {
    run();
}
