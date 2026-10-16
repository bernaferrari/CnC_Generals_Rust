use super::*;
use crate::presentation_frame::SessionView;
#[test]
fn cached_bounds_preserve_source_inversion_widening_and_invalidation() {
    let mut owner = super::super::SessionCamera::default();
    let mut peer = super::super::SessionCamera::default();
    let map = (Vec3::ZERO, Vec3::new(1000., 0., 800.));
    owner.constraints.recalculate(map.0, map.1, 30.).unwrap();
    peer.constraints.recalculate(map.0, map.1, 80.).unwrap();
    owner.scripted_camera_constraint_widen = Some((-50., 1500., -20., 900.));
    assert_eq!(
        owner
            .constraint_bounds()
            .unwrap()
            .constrain(Vec3::new(2000., 17., -100.)),
        Vec3::new(1500., 17., -20.)
    );
    let cached = owner.constraints;
    owner.camera_yaw_radians = 1.;
    owner.camera_pitch_radians = 0.2;
    owner.camera_zoom = 2.;
    owner.native_camera.set_follow_ground(80.);
    let mut view = SessionView {
        camera: owner,
        target: Vec3::new(40., 0., 50.),
        ..Default::default()
    };
    view.set_camera_transform(map, &mut |_| unreachable!())
        .unwrap();
    let mut owner = view.camera;
    assert_eq!(owner.constraints, cached);
    owner.invalidate_constraints();
    owner.rebuild_projection();
    let mut view = SessionView {
        camera: owner,
        target: Vec3::new(40., 0., 50.),
        ..Default::default()
    };
    view.set_camera_transform(map, &mut |_| unreachable!())
        .unwrap();
    let owner = view.camera;
    assert_eq!(owner.scripted_camera_constraint_widen, None);
    assert_ne!(owner.constraints, cached);
    assert_eq!(peer.constraint_bounds().unwrap().x_min, 80.);
    let inverted = ConstraintBounds::for_map(Vec3::ZERO, Vec3::new(10., 0., 10.), 20.);
    inverted.validate().unwrap();
    assert_eq!(
        inverted.constrain(Vec3::new(5., 7., 5.)),
        Vec3::new(-10., 7., -10.)
    );
    assert!(
        ConstraintBounds {
            x_min: f32::NAN,
            ..inverted
        }
        .validate()
        .is_err()
    );
}
#[test]
fn logical_and_shake_local_clamps_are_distinct_and_move_eye_and_look_together() {
    let mut view = SessionView::default();
    view.camera
        .constraints
        .recalculate(Vec3::ZERO, Vec3::new(1000., 0., 1000.), 30.)
        .unwrap();
    view.target = Vec3::new(2000., 17., -100.);
    view.impulse.offset = Vec3::new(100., 0., -50.);
    let bounds = view.camera.constraint_bounds().unwrap();
    view.target = bounds.constrain(view.target);
    let canonical = view.target;
    view.commit_orbit_pose();
    let local = bounds.constrain(canonical + view.impulse.offset);
    let (eye, look) = view.camera.native_camera.basis().pose(local, 1., 0., 0.);
    assert_eq!(view.camera.camera_position, eye);
    assert_eq!(
        view.camera.view_matrix,
        glam::Mat4::look_at_rh(eye, look, Vec3::Y)
    );
    assert_eq!(view.target, canonical);
    view.impulse.offset = Vec3::new(-10., 0., 10.);
    view.commit_orbit_pose();
    let local = canonical + view.impulse.offset;
    let (eye, look) = view.camera.native_camera.basis().pose(local, 1., 0., 0.);
    assert_eq!(view.camera.camera_position, eye);
    assert_eq!(
        view.camera.view_matrix,
        glam::Mat4::look_at_rh(eye, look, Vec3::Y)
    );
    let saved = serde_json::to_vec(&view.camera.constraints).unwrap();
    let shakers = view.camera.script_camera_shakers.len();
    for _ in 0..120 {
        view.commit_orbit_pose();
    }
    assert_eq!(serde_json::to_vec(&view.camera.constraints).unwrap(), saved);
    assert_eq!(view.camera.script_camera_shakers.len(), shakers);
}

#[test]
fn invalid_cache_preserves_raw_look_at_until_new_provisional_geometry_is_installed() {
    let mut view = SessionView::default();
    view.camera.rebuild_projection();
    view.camera.camera_zoom = 3.;
    view.commit_orbit_pose();
    let map = (Vec3::ZERO, Vec3::new(10000., 0., 10000.));
    let old_inset = view.camera.viewport.constraint_inset(
        view.camera.view_matrix,
        view.camera.projection_matrix,
        0.,
    );
    let requested = Vec3::new(5., 0., 5000.);
    let stale = ConstraintBounds::for_map(map.0, map.1, old_inset).constrain(requested);
    view.camera.camera_zoom = 0.2;
    view.camera.invalidate_constraints();
    assert_eq!(view.camera.constrain_target_if_cached(requested), requested);
    let mut data = gamelogic::system::map_loader::MapData::new();
    data.width = 32;
    data.height = 32;
    data.heightmap = vec![0; 32 * 32];
    let mut terrain = gamelogic::terrain::TerrainLogic::new();
    terrain.load_map_data(data);
    let projected =
        crate::presentation_frame::receiving_look_at_target(requested, &view.camera, &terrain)
            .unwrap();
    assert_eq!(
        projected, requested,
        "raw terrain observation is before constraints"
    );
    let target = view.receiving_look_at(requested, map, &terrain).unwrap();
    let cache = view.camera.constraint_bounds().unwrap();
    assert_eq!(target, cache.constrain(projected));
    assert!(
        target.x < stale.x,
        "new provisional view permits an edge target lost by stale preclamp: {target:?} vs {stale:?}"
    );
    let cache_before = view.camera.constraints;
    view.camera.view_matrix = glam::Mat4::IDENTITY;
    view.camera.camera_zoom = 5.;
    assert_eq!(
        view.camera.constrain_target_if_cached(requested),
        cache.constrain(requested)
    );
    assert_eq!(
        view.camera.constraints, cache_before,
        "valid readers never recompute from changed matrices"
    );
}
