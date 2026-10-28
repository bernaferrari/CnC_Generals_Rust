use super::*;
fn receiving_actor(id: ObjectId, mode: LocomotorBehaviorZ) -> Object {
    use game_engine::common::ini::ini_locomotor::{
        LocomotorAppearance as Appearance, LocomotorBehaviorZ as Z, LocomotorSurfaceTypeMask,
        LocomotorTemplate,
    };
    let mut template = LocomotorTemplate::new("ActualTypedZ".into());
    template.surfaces.add_surface(LocomotorSurfaceTypeMask::AIR);
    template.appearance = Appearance::Hover;
    template.behavior_z = match mode {
        LocomotorBehaviorZ::SurfaceRelativeHeight => Z::SurfaceRelativeHeight,
        LocomotorBehaviorZ::FixedAbsoluteHeight => Z::FixedAbsoluteHeight,
        LocomotorBehaviorZ::NoZMotiveForce => Z::NoZMotiveForce,
        _ => panic!("fixture admits only its required source modes"),
    };
    template.preferred_height = if mode == LocomotorBehaviorZ::SurfaceRelativeHeight {
        -1.
    } else {
        0.
    };
    template.lift = 0.;
    template.lift_damaged = 0.;
    template.max_speed = 1.;
    template.allow_motive_force_while_airborne = true;
    let binding =
        crate::game_logic::locomotor_bootstrap::host_locomotor_binding_from_template(&template)
            .unwrap();
    let mut actor = ThingTemplate::new("ActualTypedZActor");
    actor.add_kind_of(KindOf::Vehicle).set_health(100.);
    let mut object = Object::new(actor, id, Team::USA);
    crate::game_logic::locomotor_bootstrap::apply_host_locomotor_binding(&mut object, &binding);
    object.set_position(Vec3::new(0., 0.2, 0.));
    object.ground_height = 0.;
    object.physics_rules.gravity = -0.1;
    object.crusher_level = 2;
    object.selection_radius = 4.;
    object.owner_player_id = Some(1);
    object.stick_to_ground = false;
    object.shock_allow_bounce = false;
    object.apply_friction_2d_when_airborne = false;
    object.aerodynamic_friction = 0.;
    object.extra_friction = 0.;
    object
}
#[test]
fn actual_world_z_force_preserves_pose_until_one_source_physics_euler() {
    let id = ObjectId(740);
    let mut world = GameLogic::new();
    world.objects.insert(
        id,
        receiving_actor(id, LocomotorBehaviorZ::SurfaceRelativeHeight),
    );
    let start = world.objects[&id].get_position();
    assert!(
        world
            .apply_owned_locomotor_z_with_effects(id, None, 0, &mut Default::default())
            .unwrap()
    );
    assert_eq!(world.objects[&id].get_position(), start);
    assert_eq!(world.objects[&id].movement.velocity, Vec3::ZERO);
    assert_eq!(
        world.objects[&id].physics_accel,
        Vec3::ZERO,
        "zero authored lift remains zero force"
    );
    world
        .integrate_owned_physics_motion_with_effects(&mut Default::default())
        .unwrap();
    assert!((world.objects[&id].get_position().y - 0.1).abs() < 1e-6);
    assert!((world.objects[&id].movement.velocity.y + 0.1).abs() < 1e-6);
    assert_eq!(
        world.objects[&id].physics_previous_accel,
        Vec3::ZERO,
        "gravity is not part of source prevAccel"
    );
    assert_eq!(world.objects[&id].physics_accel, Vec3::ZERO);
}
#[test]
fn actual_world_z_controller_isolated_from_same_id_peer_lifecycle() {
    let id = ObjectId(741);
    let mut world = GameLogic::new();
    let mut reference = GameLogic::new();
    world.objects.insert(
        id,
        receiving_actor(id, LocomotorBehaviorZ::SurfaceRelativeHeight),
    );
    reference.objects.insert(
        id,
        receiving_actor(id, LocomotorBehaviorZ::SurfaceRelativeHeight),
    );
    let mut peer = GameLogic::new();
    let mut actor = receiving_actor(id, LocomotorBehaviorZ::FixedAbsoluteHeight);
    actor.loco_preferred_height = 40.;
    peer.objects.insert(id, actor);
    peer.apply_owned_locomotor_z_with_effects(id, None, 0, &mut Default::default())
        .unwrap();
    assert_eq!(peer.objects[&id].get_position().y, 40.);
    peer.reset();
    drop(peer);
    for receiver in [&mut world, &mut reference] {
        receiver
            .apply_owned_locomotor_z_with_effects(id, None, 0, &mut Default::default())
            .unwrap();
        receiver
            .integrate_owned_physics_motion_with_effects(&mut Default::default())
            .unwrap();
    }
    assert_eq!(
        world.objects[&id].get_position(),
        reference.objects[&id].get_position()
    );
    assert_eq!(
        world.objects[&id].movement.velocity,
        reference.objects[&id].movement.velocity
    );
}
#[cfg(feature = "game_client")]
#[test]
fn source_z_only_snap_has_no_tree_event_and_horizontal_physics_motion_does() {
    use game_engine::common::{
        audio::{AudioEventRts, AudioSubmissionLocality},
        ini::ini_fx_list::{FXList, FXNugget},
    };
    struct Audio(Vec<String>);
    impl NativeAudioEffects for Audio {
        fn add_audio_event(&mut self, event: &AudioEventRts, _: &AudioSubmissionLocality) {
            self.0.push(event.get_event_name().to_owned())
        }
    }
    for mode in [
        LocomotorBehaviorZ::SurfaceRelativeHeight,
        LocomotorBehaviorZ::FixedAbsoluteHeight,
    ] {
        let id = ObjectId(742);
        let mut world = GameLogic::new();
        world.add_player(Player::new(1, Team::USA, "Local", true));
        world
            .world_services
            .shroud()
            .lock()
            .unwrap()
            .init_shroud_grid(100., 100.);
        world
            .world_services
            .shroud()
            .lock()
            .unwrap()
            .reveal_map_for_player_permanently(1)
            .unwrap();
        let mut definition = FXList::new("SourceZTreeFX".into());
        definition.add_nugget(FXNugget::Sound {
            name: "SourceZTreeSound".into(),
        });
        let mut catalog = game_client::fx_list::FXListStore::new();
        catalog.add_fx_list(
            "SourceZTreeFX".into(),
            game_client::fx_list::FXList::from_definition(&definition),
        );
        world.admit_fx_catalog(catalog);
        let mut object = receiving_actor(id, mode);
        if mode == LocomotorBehaviorZ::SurfaceRelativeHeight {
            // Object.cpp2563-2588: terrain notification requires a changed
            // integer HORIZONTAL position. A pure Z-height step cannot topple.
            object.movement.velocity.x = 4.;
        }
        world.objects.insert(id, object);
        let mut terrain = game_client::terrain::terrain_visual::TerrainVisualImpl::new();
        terrain
            .tree_buffer_mut()
            .set_bounds(game_client::terrain::TreeRegion2D::new(
                glam::Vec2::ZERO,
                glam::Vec2::splat(100.),
            ));
        terrain
            .tree_buffer_mut()
            .add_tree(
                702,
                Vec3::new(1., 0., 0.),
                1.,
                0.,
                1.,
                game_client::terrain::TreeModuleData {
                    model_name: "SourceZTree".into(),
                    do_topple: true,
                    topple_fx: Some("SourceZTreeFX".into()),
                    ..Default::default()
                },
                game_client::terrain::TreeSphere {
                    center: Vec3::ZERO,
                    radius: 2.,
                },
            )
            .unwrap();
        let start = world.objects[&id].get_position();
        let mut audio = Audio(Vec::new());
        world
            .apply_owned_locomotor_z_with_effects(
                id,
                None,
                0,
                &mut ScriptPresentationEffects {
                    terrain: Some(&mut terrain),
                    audio: Some(&mut audio),
                    ..Default::default()
                },
            )
            .unwrap();
        if mode == LocomotorBehaviorZ::FixedAbsoluteHeight {
            assert_eq!(world.objects[&id].get_position().y, 0.);
            assert!(
                audio.0.is_empty(),
                "C++ pure height snap does not change the integer horizontal trigger position"
            );
        } else {
            assert_eq!(world.objects[&id].get_position(), start);
            assert!(audio.0.is_empty());
            world
                .integrate_owned_physics_motion_with_effects(&mut ScriptPresentationEffects {
                    terrain: Some(&mut terrain),
                    audio: Some(&mut audio),
                    ..Default::default()
                })
                .unwrap();
            assert_eq!(
                audio.0,
                ["SourceZTreeSound"],
                "actual horizontal Physics pose crosses the source integer gate"
            );
        }
        assert!(terrain.tree_buffer_mut().pending_fx_events().is_empty());
    }
}

#[test]
fn hover_appearance_does_not_invent_a_source_z_motive_controller() {
    let id = ObjectId(743);
    let mut world = GameLogic::new();
    world
        .objects
        .insert(id, receiving_actor(id, LocomotorBehaviorZ::NoZMotiveForce));
    world
        .apply_owned_locomotor_z_with_effects(id, None, 0, &mut Default::default())
        .unwrap();
    assert_eq!(
        world.objects[&id].loco_behavior_z,
        LocomotorBehaviorZ::NoZMotiveForce
    );
    assert_eq!(world.objects[&id].physics_accel, Vec3::ZERO);
    assert_eq!(world.objects[&id].get_position().y, 0.2);
}
#[test]
fn source_post_transform_reset_keeps_bounce_for_the_next_actual_physics_phase() {
    let id = ObjectId(744);
    let mut object = receiving_actor(id, LocomotorBehaviorZ::SurfaceRelativeHeight);
    object.set_position(Vec3::ZERO);
    object.movement.velocity = Vec3::ZERO;
    object.physics_accel = Vec3::Y * 0.7;
    object.shock_allow_bounce = true;
    let force = Vec3::Y * (object.physics_get_mass() * 2.);
    let mut events = crate::game_logic::SessionAudio::default();
    let _ = object.step_motion_contact_with_effects(
        crate::game_logic::object::MotionContact::AfterPosition {
            ground_y: 0.,
            new_pos: Vec3::ZERO,
            old_y: 0.2,
            impact_vy: -0.1,
            bounce_force: Some(force),
        },
        false,
        0,
        None,
        &mut Default::default(),
        &mut ObjectAudio::queue(&mut events),
    );
    assert_eq!(
        object.movement.velocity,
        Vec3::ZERO,
        "bounce is not another same-frame velocity integration"
    );
    assert!(
        (object.physics_accel.y - 2.).abs() < 1e-6,
        "source post-transform reset discards old/callback force before bounce admission"
    );
    let mut world = GameLogic::new();
    world.objects.insert(id, object);
    world
        .integrate_owned_physics_motion_with_effects(&mut Default::default())
        .unwrap();
    assert!((world.objects[&id].movement.velocity.y - 1.9).abs() < 1e-5);
    assert!((world.objects[&id].get_position().y - 1.9).abs() < 1e-5);
    assert!((world.objects[&id].physics_previous_accel.y - 2.).abs() < 1e-5);
}

#[test]
fn actual_native_marching_does_not_add_a_fractional_second_vertical_euler() {
    let id = ObjectId(745);
    let mut world = GameLogic::new();
    let mut actor = receiving_actor(id, LocomotorBehaviorZ::SurfaceRelativeHeight);
    actor.set_position(Vec3::new(20., 10., 20.));
    actor.movement.velocity = Vec3::new(0., -0.2, 0.);
    actor.movement.target_position = Some(Vec3::new(120., 10., 20.));
    actor.locomotor_goal_type = LocoGoalType::PositionExplicit;
    world.objects.insert(id, actor);
    world
        .update_movement_locomotor_pass_with_effects(&[id], 1. / 30., &mut Default::default())
        .unwrap();
    assert!(
        (world.objects[&id].get_position().y - 10.).abs() < 1e-6,
        "AI must neither full-step Y nor apply velocity.y*dt from horizontal march"
    );
    assert!((world.objects[&id].movement.velocity.y + 0.2).abs() < 1e-6);
    world
        .integrate_owned_physics_motion_with_effects(&mut Default::default())
        .unwrap();
    assert!((world.objects[&id].get_position().y - 9.7).abs() < 1e-5);
    assert!((world.objects[&id].movement.velocity.y + 0.3).abs() < 1e-5);
}
