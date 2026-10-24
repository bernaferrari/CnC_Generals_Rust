use super::super::installed_draw_pose::{InstalledDrawObject, InstalledDrawPose};
use super::*;
use crate::assets::LiveDrawPlaybackIdentity;

fn identity(world: &Arc<()>, object: &Arc<()>, generation: u64) -> LiveDrawPlaybackIdentity {
    LiveDrawPlaybackIdentity::new(Arc::clone(world), Arc::clone(object), 7, 42, generation)
        .with_native_pose(1, Mat4::from_translation(Vec3::new(3., 4., 5.)))
}
fn key(generation: u64) -> game_client::core::PresentationDirectDrawableBindingKey {
    game_client::core::PresentationDirectDrawableBindingKey {
        host_epoch: 17,
        object_id: 42,
        drawable_id: game_client::core::DrawableId(80),
        binding_generation: generation,
    }
}
fn admitted(identity: LiveDrawPlaybackIdentity) -> ObjectVisualState {
    ObjectVisualState {
        native_host_identity: Some(identity),
        native_client_binding: Some(key(3)),
        native_source_template: "CameraOwner".into(),
        native_source_supported: true,
        native_condition_supported: true,
        installed_draw_pose: InstalledDrawObject::NoRenderObject,
        native_pending_condition: Some(crate::assets::AuthoredDrawModel::default()),
        ..Default::default()
    }
}
#[test]
fn same_id_peer_reset_and_replacement_cannot_query_or_consume_installed_owner() {
    let world = Arc::new(());
    let object = Arc::new(());
    let actual = identity(&world, &object, 9);
    let peer = identity(&Arc::new(()), &Arc::new(()), 9);
    let replacement = identity(&world, &Arc::new(()), 10);
    let reset = identity(&Arc::new(()), &object, 9);
    let mut states = HashMap::from([((42, 2), admitted(actual.clone()))]);
    let before = states.clone();
    let mut failed = pipeline_collect::FailedModelResolutions::default();
    let mut pending = None;
    let mut imports = HashMap::new();
    let mut playback = HashMap::new();
    let mut lifetimes: HashMap<_, _> = states
        .iter()
        .filter_map(|((object, _), state)| {
            state.native_host_identity.as_ref().map(|identity| {
                (
                    *object,
                    native_draw_world::NativeDrawableLifetime {
                        identity: identity.clone(),
                        binding: state.native_client_binding,
                    },
                )
            })
        })
        .collect();
    {
        let mut loan = NativeInstalledPoseLoan {
            native_drawable_lifetimes: &mut lifetimes,
            live_draw_playback: &mut playback,
            drawable_visual_states: &mut states,
            failed_model_resolutions: &mut failed,
            pending_client_drawable_restore: &mut pending,
            pending_client_drawable_imports: &mut imports,
            animation_sync_ms: 999,
        };
        for foreign in [&peer, &replacement, &reset] {
            assert_eq!(
                loan.installed_camera_pose_for(foreign, key(3), 2, "CAMERA"),
                Err(NativeCameraPoseUnavailable::UnadmittedDrawableLifecycle)
            );
            assert_eq!(
                *loan.drawable_visual_states, before,
                "identity rejection must precede dirty, restore and lazy animation effects"
            );
        }
        assert_eq!(
            loan.installed_camera_pose_for(&actual, key(3), 2, "CAMERA"),
            Err(NativeCameraPoseUnavailable::NoRenderObject),
            "the admitted owner's actual missing RenderObj is not a foreign identity"
        );
    }
    assert_eq!(states, before);
    assert!(pending.is_none());
    assert!(imports.is_empty());
}
#[test]
fn residual_same_id_sibling_from_prior_generation_rejects_whole_source_query() {
    let world = Arc::new(());
    let object = Arc::new(());
    let actual = identity(&world, &object, 9);
    let stale = identity(&world, &Arc::new(()), 8);
    let mut states = HashMap::from([
        ((42, 2), admitted(actual.clone())),
        ((42, 5), admitted(stale)),
    ]);
    let before = states.clone();
    let mut failed = pipeline_collect::FailedModelResolutions::default();
    let mut pending = None;
    let mut imports = HashMap::new();
    let mut playback = HashMap::new();
    let mut lifetimes: HashMap<_, _> = states
        .iter()
        .filter_map(|((object, _), state)| {
            state.native_host_identity.as_ref().map(|identity| {
                (
                    *object,
                    native_draw_world::NativeDrawableLifetime {
                        identity: identity.clone(),
                        binding: state.native_client_binding,
                    },
                )
            })
        })
        .collect();
    let loan = NativeInstalledPoseLoan {
        native_drawable_lifetimes: &mut lifetimes,
        live_draw_playback: &mut playback,
        drawable_visual_states: &mut states,
        failed_model_resolutions: &mut failed,
        pending_client_drawable_restore: &mut pending,
        pending_client_drawable_imports: &mut imports,
        animation_sync_ms: 999,
    };
    assert_eq!(
        loan.validate_identity(&actual, 2),
        Err(NativeCameraPoseUnavailable::UnadmittedDrawableLifecycle)
    );
    assert_eq!(*loan.drawable_visual_states, before);
}
#[test]
fn current_transform_uses_same_installed_owner_without_another_animation_clock() {
    let world = Arc::new(());
    let object = Arc::new(());
    let actual = identity(&world, &object, 9);
    let selected = crate::assets::AuthoredDrawModel::default();
    let mut state = admitted(actual.clone());
    state.installed_draw_pose = InstalledDrawObject::Ready(InstalledDrawPose::created(
        Arc::new(W3DModel::new("ActualMesh".into())),
        Mat4::IDENTITY,
        &selected,
        None,
    ));
    let current = actual.with_native_pose(2, Mat4::from_translation(Vec3::new(12., 34., 56.)));
    state.receive_native_transform_observations(&current, 1., 0., 0., false, 1., 100.);
    let InstalledDrawObject::Ready(pose) = &state.installed_draw_pose else {
        panic!("actual installed model lost");
    };
    assert_eq!(pose.world, Mat4::from_translation(Vec3::new(12., 34., 56.)));
    assert_eq!(pose.sampled_frame, 0.);
    assert!(state.animation.is_none());
    assert_eq!(state.native_transform_revision, Some(2));
    assert_eq!(state.native_host_identity.as_ref(), Some(&current));
}

#[test]
fn same_object_same_template_client_replacement_requires_actual_constructor_before_query() {
    let world = Arc::new(());
    let object = Arc::new(());
    let actual = identity(&world, &object, 9);
    let mut states = HashMap::from([((42, 2), admitted(actual.clone()))]);
    let before = states.clone();
    let mut failed = pipeline_collect::FailedModelResolutions::default();
    let mut pending = None;
    let mut imports = HashMap::new();
    let mut playback = HashMap::new();
    let mut lifetimes: HashMap<_, _> = states
        .iter()
        .filter_map(|((object, _), state)| {
            state.native_host_identity.as_ref().map(|identity| {
                (
                    *object,
                    native_draw_world::NativeDrawableLifetime {
                        identity: identity.clone(),
                        binding: state.native_client_binding,
                    },
                )
            })
        })
        .collect();
    let mut loan = NativeInstalledPoseLoan {
        native_drawable_lifetimes: &mut lifetimes,
        live_draw_playback: &mut playback,
        drawable_visual_states: &mut states,
        failed_model_resolutions: &mut failed,
        pending_client_drawable_restore: &mut pending,
        pending_client_drawable_imports: &mut imports,
        animation_sync_ms: 999,
    };
    assert_eq!(
        loan.validate_binding(&actual, key(4), 2),
        Err(NativeCameraPoseUnavailable::UnadmittedDrawableLifecycle)
    );
    assert!(
        loan.admit_binding(&actual, key(4)).is_err(),
        "a new Client key cannot relabel a retained RenderObj"
    );
    assert_eq!(*loan.drawable_visual_states, before);
    // This is the real metadata reset performed by the constructor producer.
    let state = loan.drawable_visual_states.get_mut(&(42, 2)).unwrap();
    state.native_client_binding = None;
    loan.native_drawable_lifetimes.get_mut(&42).unwrap().binding = None;
    assert_eq!(
        loan.validate_binding(&actual, key(4), 2),
        Err(NativeCameraPoseUnavailable::UnadmittedDrawableLifecycle),
        "constructor-before-key is unavailable"
    );
    loan.admit_binding(&actual, key(4)).unwrap();
    assert!(loan.validate_binding(&actual, key(4), 2).is_ok());
    assert_eq!(
        loan.validate_binding(&actual, key(3), 2),
        Err(NativeCameraPoseUnavailable::UnadmittedDrawableLifecycle)
    );
}

#[test]
fn native_destroy_loan_retires_only_its_actual_render_and_playback_owner() {
    use super::super::native_draw_world::NativeDrawWorld;
    let actual = identity(&Arc::new(()), &Arc::new(()), 9);
    let peer_identity = identity(&Arc::new(()), &Arc::new(()), 9);
    let sibling = LiveDrawPlaybackIdentity::new(Arc::new(()), Arc::new(()), 7, 43, 1);
    let mut world = NativeDrawWorld::default();
    world.drawable_visual_states = HashMap::from([
        ((42, 2), admitted(actual.clone())),
        ((43, 2), admitted(sibling.clone())),
    ]);
    world.live_draw_playback = HashMap::from([
        (
            actual.playback_key(2),
            crate::assets::LiveDrawPlayback::default(),
        ),
        (
            sibling.playback_key(2),
            crate::assets::LiveDrawPlayback::default(),
        ),
    ]);
    let mut peer = NativeDrawWorld::default();
    peer.drawable_visual_states
        .insert((42, 2), admitted(peer_identity.clone()));
    peer.live_draw_playback
        .insert(peer_identity.playback_key(2), Default::default());
    let peer_before = peer.drawable_visual_states.clone();
    let mut failed = pipeline_collect::FailedModelResolutions::default();
    world
        .borrow_installed_poses(&mut failed, 999)
        .retire_native_drawable(crate::game_logic::ObjectId(42));
    assert!(!world.drawable_visual_states.contains_key(&(42, 2)));
    assert!(world.drawable_visual_states.contains_key(&(43, 2)));
    assert!(
        !world
            .live_draw_playback
            .contains_key(&actual.playback_key(2))
    );
    assert!(
        world
            .live_draw_playback
            .contains_key(&sibling.playback_key(2))
    );
    assert_eq!(peer.drawable_visual_states, peer_before);
    assert_eq!(peer.live_draw_playback.len(), 1);
    assert!(
        peer.live_draw_playback
            .contains_key(&peer_identity.playback_key(2))
    );
    assert!(world.pending_client_drawable_restore.is_none());
    assert!(world.pending_client_drawable_imports.is_empty());
}

#[test]
fn installed_transform_visits_use_each_matrix_at_same_revision_and_reject_foreign_birth() {
    use super::super::native_draw_source::NativeDrawSource;
    let world = Arc::new(());
    let actual = identity(&world, &Arc::new(()), 9);
    let foreign = identity(&Arc::new(()), &Arc::new(()), 9);
    let stale = identity(&world, &Arc::new(()), 8);
    let selected = crate::assets::AuthoredDrawModel::default();
    let mut state = admitted(actual.clone());
    state.installed_draw_pose = InstalledDrawObject::Ready(InstalledDrawPose::created(
        Arc::new(W3DModel::new("ActualMesh".into())),
        Mat4::IDENTITY,
        &selected,
        None,
    ));
    let mut peer = admitted(foreign.clone());
    let peer_before = peer.clone();
    fn source(identity: &LiveDrawPlaybackIdentity) -> NativeDrawSource<'_> {
        NativeDrawSource {
            identity: Some(identity),
            receiving: None,
            template: "CameraOwner",
            mesh_scale: 1.,
            float_yaw: 0.,
            float_pitch: 0.,
            under_construction: false,
            construction_percent: 1.,
            max_height_above_position: 100.,
        }
    }
    let first = Mat4::from_translation(Vec3::new(11., 13., 17.));
    let second = first * Mat4::from_rotation_y(0.7);
    for expected in [first, second, second] {
        let InstalledDrawObject::Ready(pose) = &mut state.installed_draw_pose else {
            panic!()
        };
        pose.hierarchy_valid = true;
        source(&actual).install_observed_transform(
            &mut state,
            game_client::drawable::Matrix4 {
                elements: expected.transpose().to_cols_array_2d(),
            },
        );
        let InstalledDrawObject::Ready(pose) = &state.installed_draw_pose else {
            panic!()
        };
        assert_eq!(pose.world, expected);
        assert!(
            !pose.hierarchy_valid,
            "every Set_Transform invalidates, including equal visits"
        );
        assert_eq!(state.native_transform_revision, Some(1));
        assert!(state.animation.is_none());
    }
    let before = state.clone();
    for rejected in [&foreign, &stale] {
        source(rejected)
            .install_observed_transform(&mut state, game_client::drawable::Matrix4::identity());
        assert_eq!(state, before);
    }
    source(&actual)
        .install_observed_transform(&mut peer, game_client::drawable::Matrix4::identity());
    assert_eq!(peer, peer_before);
    // A later frozen observation must not overwrite the event-time installed pose.
    state.receive_native_transform_observations(&actual, 1., 0., 0., false, 1., 100.);
    assert_eq!(state, before);
}
