use super::*;
fn thrust_actor(id: ObjectId) -> Object {
    let mut template = ThingTemplate::new("ThrustSourcePhaseVehicle");
    template.add_kind_of(KindOf::Vehicle).set_health(100.0);
    let mut object = Object::new(template, id, Team::USA);
    object.set_position(glam::Vec3::new(0.0, 3.0, 0.0));
    use game_engine::common::ini::ini_locomotor::{
        LocomotorAppearance as A, LocomotorBehaviorZ as Z, LocomotorSurfaceTypeMask,
        LocomotorTemplate,
    };
    let mut source = LocomotorTemplate::new("ActualThrustSource".into());
    source.surfaces.add_surface(LocomotorSurfaceTypeMask::AIR);
    source.appearance = A::Thrust;
    source.behavior_z = Z::NoZMotiveForce;
    source.allow_motive_force_while_airborne = true;
    source.stick_to_ground = false;
    source.max_speed = 1.;
    source.max_speed_damaged = 1.;
    source.acceleration = 1.;
    source.acceleration_damaged = 1.;
    source.max_turn_rate = std::f32::consts::PI;
    source.max_turn_rate_damaged = std::f32::consts::PI;
    source.min_speed = 0.;
    source.braking = 0.;
    source.max_thrust_angle = std::f32::consts::PI;
    let binding =
        crate::game_logic::locomotor_bootstrap::host_locomotor_binding_from_template(&source)
            .unwrap();
    crate::game_logic::locomotor_bootstrap::apply_host_locomotor_binding(&mut object, &binding);
    object.movement.velocity = glam::Vec3::ZERO;
    object.aerodynamic_friction = 0.0;
    object.extra_friction = 0.0;
    object.apply_friction_2d_when_airborne = false;
    object.physics_rules.gravity = 0.0;
    object
}
#[test]
fn thrust_cpp_force_only_leaves_velocity_and_translation_until_physics() {
    let id = ObjectId(731);
    let mut world = crate::game_logic::GameLogic::new();
    let mut object = thrust_actor(id);
    let start = object.get_position();
    let goal = start + glam::Vec3::X * 1000.0;
    object.movement.target_position = Some(goal);
    object.move_towards_thrust(goal, 99999.0, 30.0, 1.0 / 30.0);
    assert_eq!(
        object.get_position(),
        start,
        "locomotor must not Euler position"
    );
    assert_eq!(
        object.movement.velocity,
        glam::Vec3::ZERO,
        "force is pending until Physics"
    );
    // Source: speed1/frame, accel1/frame², zero velocity => force/mass1.
    // Host accumulator delta-v converts1/frame to30/second exactly once.
    assert!((object.physics_accel - glam::Vec3::X * 30.0).length() < 0.0001);
    world.objects.insert(id, object);
    world.integrate_owned_physics_motion_with_effects(&mut Default::default());
    let actual = world.objects.get(&id).unwrap();
    assert!((actual.movement.velocity - glam::Vec3::X * 30.0).length() < 0.0001);
    assert!(
        (actual.get_position() - (start + glam::Vec3::X)).length() < 0.0001,
        "one source-frame Physics Euler despite persistent move goal"
    );
    assert_eq!(actual.physics_accel, glam::Vec3::ZERO);
}
#[test]
fn thrust_cpp_orients_in_3d_without_pivot_translation() {
    let mut object = thrust_actor(ObjectId(732));
    object.movement.velocity = glam::Vec3::new(0.0, 30.0, 30.0);
    object.turn_pivot_offset = 100.0;
    let start = object.get_position();
    let goal = start + glam::Vec3::X * 1000.0;
    object.move_towards_thrust(goal, 99999.0, 30.0, 1.0 / 30.0);
    assert_eq!(object.get_position(), start);
    let forward = object.get_transform_matrix().x_axis.truncate();
    let expected = glam::Vec3::new(0.0, 1.0, 1.0).normalize();
    assert!(
        (forward - expected).length() < 0.0001,
        "source buildTransformMatrix follows full velocity, including pitch"
    );
    assert_eq!(object.movement.velocity, glam::Vec3::new(0.0, 30.0, 30.0));
}
#[cfg(feature = "game_client")]
#[test]
fn thrust_tree_sound_is_at_receiving_physics_euler_not_locomotor_force() {
    use game_engine::common::{
        audio::{AudioEventRts, AudioSubmissionLocality},
        ini::ini_fx_list::{FXList, FXNugget},
    };
    struct Audio(Vec<String>);
    impl crate::game_logic::NativeAudioEffects for Audio {
        fn add_audio_event(&mut self, event: &AudioEventRts, _: &AudioSubmissionLocality) {
            self.0.push(event.get_event_name().to_owned());
        }
    }
    let id = ObjectId(733);
    let mut world = crate::game_logic::GameLogic::new();
    world.add_player(crate::game_logic::Player::new(1, Team::USA, "Local", true));
    world
        .world_services
        .shroud()
        .lock()
        .unwrap()
        .init_shroud_grid(100.0, 100.0);
    world
        .world_services
        .shroud()
        .lock()
        .unwrap()
        .reveal_map_for_player_permanently(1)
        .unwrap();
    let mut definition = FXList::new("ThrustTreeFX".into());
    definition.add_nugget(FXNugget::Sound {
        name: "ThrustTreeSound".into(),
    });
    let mut catalog = game_client::fx_list::FXListStore::new();
    catalog.add_fx_list(
        "ThrustTreeFX".into(),
        game_client::fx_list::FXList::from_definition(&definition),
    );
    world.admit_fx_catalog(catalog);
    let mut object = thrust_actor(id);
    object.crusher_level = 2;
    object.owner_player_id = Some(1);
    object.selection_radius = 4.0;
    let start = object.get_position();
    let goal = start + glam::Vec3::X * 1000.0;
    object.movement.target_position = Some(goal);
    world.objects.insert(id, object);
    let mut terrain = game_client::terrain::terrain_visual::TerrainVisualImpl::new();
    terrain
        .tree_buffer_mut()
        .set_bounds(game_client::terrain::TreeRegion2D::new(
            glam::Vec2::ZERO,
            glam::Vec2::splat(100.0),
        ));
    terrain
        .tree_buffer_mut()
        .add_tree(
            701,
            glam::Vec3::new(1.0, 0.0, 0.0),
            1.0,
            0.0,
            1.0,
            game_client::terrain::TreeModuleData {
                model_name: "ThrustTree".into(),
                do_topple: true,
                topple_fx: Some("ThrustTreeFX".into()),
                ..Default::default()
            },
            game_client::terrain::TreeSphere {
                center: glam::Vec3::ZERO,
                radius: 2.0,
            },
        )
        .unwrap();
    let mut audio = Audio(Vec::new());
    assert!(world.move_owned_thrust_with_effects(
        id,
        goal,
        99999.,
        30.,
        0,
        false,
        0.,
        &mut crate::game_logic::ScriptPresentationEffects {
            terrain: Some(&mut terrain),
            audio: Some(&mut audio),
            ..Default::default()
        }
    ));
    assert!(audio.0.is_empty());
    assert_eq!(world.objects.get(&id).unwrap().get_position(), start);
    world.integrate_owned_physics_motion_with_effects(
        &mut crate::game_logic::ScriptPresentationEffects {
            terrain: Some(&mut terrain),
            audio: Some(&mut audio),
            ..Default::default()
        },
    );
    assert_eq!(audio.0, ["ThrustTreeSound"]);
    assert_eq!(
        terrain.tree_buffer_mut().trees()[0].topple_state,
        game_client::terrain::W3DToppleState::Falling
    );
    assert!(terrain.tree_buffer_mut().pending_fx_events().is_empty());
}

#[test]
fn source_thrust_continuation_rereads_current_physics_velocity_after_orientation() {
    let mut actor = thrust_actor(ObjectId(734));
    actor.movement.velocity = glam::Vec3::new(0., 30., 30.);
    let goal = actor.get_position() + glam::Vec3::X * 1000.;
    let mut step =
        actor.prepare_thrust_step(goal, 99999., 30., 0, false, 0., &mut Default::default());
    let (direction, acceleration, max_speed) = step.force.expect("source thrust force condition");
    if let Some(reaction) = step.reaction.take() {
        assert!(!reaction.has_tree_fx());
        actor.finish_position_reaction(reaction);
    }
    // Explicit continuation witness; this is not a claim that native Sound
    // currently permits hostile safe mutation of the registered actor.
    actor.movement.velocity = glam::Vec3::new(9., 12., 15.);
    let damping = (acceleration / max_speed).clamp(0., 1.);
    let expected = (direction * acceleration - actor.movement.velocity / 30. * damping) * 30.;
    actor.finish_owned_thrust_step(step);
    assert!((actor.physics_accel - expected).length() < 0.0001);
}
#[test]
fn actual_owned_thrust_orientation_and_force_ignore_same_id_peer_lifecycle() {
    let id = ObjectId(735);
    let mut world = crate::game_logic::GameLogic::new();
    let mut alone = crate::game_logic::GameLogic::new();
    let mut actor = thrust_actor(id);
    actor.movement.velocity = glam::Vec3::new(0., 30., 30.);
    let start = actor.get_position();
    let goal = start + glam::Vec3::X * 1000.;
    let mut alone_actor = thrust_actor(id);
    alone_actor.movement.velocity = actor.movement.velocity;
    alone.objects.insert(id, alone_actor);
    world.objects.insert(id, actor);
    let mut peer = crate::game_logic::GameLogic::new();
    peer.objects.insert(id, thrust_actor(id));
    peer.reset();
    drop(peer);
    for receiving in [&mut world, &mut alone] {
        assert!(receiving.move_owned_thrust_with_effects(
            id,
            goal,
            99999.,
            30.,
            0,
            false,
            0.,
            &mut Default::default()
        ));
    }
    assert_eq!(world.objects[&id].get_position(), start);
    assert_eq!(
        world.objects[&id].get_transform_matrix(),
        alone.objects[&id].get_transform_matrix()
    );
    assert_eq!(
        world.objects[&id].physics_accel,
        alone.objects[&id].physics_accel
    );
    assert_ne!(world.objects[&id].physics_accel, glam::Vec3::ZERO);
}
