//! Actual PhysicsUpdate callers -> authored Transition/Die nuggets -> owned View.
use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
use crate::game_logic::host_fx_list_die::HostFxListDieData;
use crate::game_logic::host_transition_damage_fx::HostTransitionDamageFxData;
use crate::game_logic::{GameLogic, KindOf, Object, ObjectId, Player, Team, ThingTemplate};
use game_engine::common::ini::ini_fx_list::{CameraShakeType, FXList, FXNugget};
use gamelogic::common::{Coord3D, Matrix3D, ObjectShroudStatus};
use gamelogic::helpers::HostFxObjectPose;
use glam::{Mat4, Quat, Vec3};
use std::sync::mpsc::{Receiver, Sender, channel};

#[derive(Debug)]
enum Observation {
    Transition(Coord3D),
    Die(HostFxObjectPose, Option<HostFxObjectPose>),
}
struct Observe(bool, Sender<Observation>);
impl game_client::fx_list::FXNugget for Observe {
    fn do_fx_pos(
        &self,
        position: Option<&Coord3D>,
        _: Option<&Matrix3D>,
        _: f32,
        _: Option<&Coord3D>,
        _: f32,
    ) {
        assert!(!self.0, "FXListDie must retain object form");
        self.1
            .send(Observation::Transition(*position.unwrap()))
            .unwrap();
    }
    fn do_fx_obj_host(&self, primary: &HostFxObjectPose, secondary: Option<&HostFxObjectPose>) {
        assert!(self.0, "TransitionDamageFX must retain positional form");
        self.1
            .send(Observation::Die(*primary, secondary.copied()))
            .unwrap();
    }
}
fn install() -> Receiver<Observation> {
    let (sent, received) = channel();
    for (name, die) in [
        ("FX_PhysicsReceivingTransition", false),
        ("FX_PhysicsReceivingDie", true),
    ] {
        let mut definition = FXList::new(name.into());
        definition.add_nugget(FXNugget::ViewShake {
            shake_type: CameraShakeType::Normal,
        });
        let mut list = game_client::fx_list::FXList::from_definition(&definition);
        list.add_fx_nugget(Box::new(Observe(die, sent.clone())));
        game_client::fx_list::get_fx_list_store_mut().add_fx_list(name.into(), list);
    }
    received
}
const VICTIM: ObjectId = ObjectId(9701);
const CRUSHER: ObjectId = ObjectId(9702);
fn world(far: bool) -> GameLogic {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::Neutral, "Neutral", false));
    world.add_player(Player::new(1, Team::USA, "Local", true));
    world.add_player(Player::new(2, Team::China, "Crusher", false));
    let mut infantry = ThingTemplate::new("PhysicsFxVictim");
    infantry.add_kind_of(KindOf::Infantry);
    infantry.set_health(100.);
    let mut object = Object::new(infantry, VICTIM, Team::USA);
    object.owner_player_id = Some(1);
    object.crushable_level = 0;
    object.has_squish_collide = false;
    object.last_damage_source = Some(CRUSHER);
    object.fx_list_die = Some(HostFxListDieData::with_fx("FX_PhysicsReceivingDie"));
    let mut transition = HostTransitionDamageFxData::default();
    transition.enabled = true;
    transition.fx_lists_for_state[HostBodyDamageType::Rubble.ordinal() as usize] =
        vec!["FX_PhysicsReceivingTransition".into()];
    object.transition_damage_fx = Some(transition);
    world.objects.insert(VICTIM, object);
    let mut tank = ThingTemplate::new("PhysicsFxCrusher");
    tank.add_kind_of(KindOf::Vehicle);
    let mut crusher = Object::new(tank, CRUSHER, Team::China);
    crusher.owner_player_id = Some(2);
    crusher.crusher_level = 1;
    crusher.set_orientation(0.);
    crusher.movement.velocity = Vec3::new(5., 0., 0.);
    crusher.set_position(Vec3::new(-20., 0., 0.));
    world.objects.insert(CRUSHER, crusher);
    if far {
        world.session_view.target = Vec3::new(
            game_engine::common::global_data::read().max_shake_range * 2.,
            0.,
            0.,
        );
    }
    reveal(&world);
    world
}
fn reveal(world: &GameLogic) {
    let mut shroud = world.world_services.shroud().lock().unwrap();
    shroud.init_shroud_grid(128., 128.);
    shroud.reveal_map_for_player_permanently(1).unwrap();
    shroud.set_host_object_shroud_status(1, VICTIM.0, ObjectShroudStatus::Clear);
    shroud.set_host_object_shroud_status(1, CRUSHER.0, ObjectShroudStatus::Shrouded);
}
fn prepare_fall(world: &mut GameLogic, shipping: bool, lethal: bool) {
    world
        .objects
        .get_mut(&CRUSHER)
        .unwrap()
        .set_position(Vec3::new(1000., 0., 1000.));
    world.objects.get_mut(&CRUSHER).unwrap().movement.velocity = Vec3::ZERO;
    let object = world.objects.get_mut(&VICTIM).unwrap();
    object.set_position(Vec3::new(0., if shipping { 1. } else { -1. }, 0.));
    object.was_airborne_last_frame = true;
    object.immune_to_falling_damage = false;
    object.movement.velocity = Vec3::new(0., -4., 0.);
    object.physics_mass = if lethal { 100. } else { 1. };
    object.fall_height_damage_factor = 1.;
    object.stick_to_ground = false;
    object.allow_to_fall = true;
}
fn assert_effects(
    events: &Receiver<Observation>,
    secondary: Option<ObjectId>,
    secondary_position: Option<Coord3D>,
) {
    let effects: Vec<_> = events.try_iter().collect();
    assert_eq!(
        effects.len(),
        2,
        "one original state transition then one authored onDie module: {effects:?}"
    );
    match &effects[0] {
        Observation::Transition(position) => assert_eq!(*position, Coord3D::new(0., 0., 0.)),
        _ => panic!("state callback must precede authored onDie"),
    }
    match &effects[1] {
        Observation::Die(primary, source) => {
            assert_eq!(primary.id, VICTIM.0);
            assert_eq!(source.as_ref().map(|pose| ObjectId(pose.id)), secondary);
            assert_eq!(
                source.as_ref().map(|pose| pose.position),
                secondary_position
            );
            if secondary == Some(VICTIM) {
                assert_eq!(
                    source.as_ref().unwrap().transform,
                    primary.transform,
                    "self source observes the current receiving object"
                );
            }
        }
        _ => panic!("FXListDie must preserve the object pose and actual request source"),
    }
}

#[test]
fn crush_zero_then_huge_keeps_inline_owned_effects_and_receiving_crusher_pose() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "crush_zero_then_huge_keeps_inline_owned_effects_and_receiving_crusher_pose",
        || {
            let events = install();
            let mut owner = world(false);
            let mut peer = world(true);
            for world in [&mut peer, &mut owner] {
                assert!(world.apply_overlap_crush_check(CRUSHER, VICTIM, false));
                assert_eq!(world.objects[&VICTIM].health.current, 100.);
                assert!(
                    events.try_recv().is_err(),
                    "zero hit must not fabricate a Body state edge"
                );
                world
                    .objects
                    .get_mut(&CRUSHER)
                    .unwrap()
                    .set_position(Vec3::new(6., 0., 0.));
                assert!(world.apply_overlap_crush_check(CRUSHER, VICTIM, false));
                assert!(world.objects[&VICTIM].status.destroyed);
                assert_effects(&events, Some(CRUSHER), Some(Coord3D::new(6., 0., 0.)));
                assert_eq!(
                    world.objects.registered_len(),
                    2,
                    "both callback participants remain installed"
                );
            }
            assert_eq!(peer.session_view.impulse.intensity, 0.);
            assert!(
                owner.session_view.impulse.intensity > 0.,
                "OLD standalone physics effects leave the receiving owner untouched"
            );
            assert_eq!(owner.session_view.impulse.offset, Vec3::ZERO);
            assert!(owner.objects[&VICTIM].pending_death_fx.is_none());
        },
    );
}

#[test]
fn movement_landing_self_fx_survives_reconstruction_without_replay_or_peer_mutation() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "movement_landing_self_fx_survives_reconstruction_without_replay_or_peer_mutation",
        || {
            let events = install();
            // Keep a real, distinguishable compatibility View installed. Its
            // target, current impulse and committed offset must all survive
            // native construction, contact, restore and peer reset untouched.
            game_client::display::view::with_tactical_view(|view| {
                use game_client::display::view::{CameraShakeType, Point3};
                let target = Point3::new(-100000., -200000., 9.);
                view.set_position(&target);
                view.reset_camera_shake();
                let mut rules = game_engine::common::global_data::GlobalData::default();
                rules.shake_normal_intensity = 1.25;
                rules.max_shake_intensity = 10.;
                rules.max_shake_range = 100.;
                view.shake_with_services(&target, CameraShakeType::Normal, &rules, &mut |_, _| {
                    0.75
                });
                view.tick_impulse_shake();
            });
            let foreign_state = || {
                game_client::display::view::with_tactical_view_ref(|view| {
                    let position = view.position();
                    let offset = view.impulse_shake_offset();
                    (
                        position.x.to_bits(),
                        position.y.to_bits(),
                        position.z.to_bits(),
                        view.camera_shake_intensity().to_bits(),
                        offset.x.to_bits(),
                        offset.y.to_bits(),
                    )
                })
            };
            let foreign_before = foreign_state();
            let mut owner = world(false);
            let mut peer = world(true);
            prepare_fall(&mut owner, false, true);
            prepare_fall(&mut peer, false, true);
            // Decode the actual PRE-contact object state into a detached
            // receiving session. Include both resident source candidates so
            // the original retained-source observation is reconstructed too.
            let pre_contact_view = owner.session_view.capture();
            let pre_contact_objects =
                [VICTIM, CRUSHER].map(|id| (id, serde_json::to_vec(&owner.objects[&id]).unwrap()));
            let mut restored = world(false);
            restored.frame = owner.frame;
            restored.session_view.admit(pre_contact_view).unwrap();
            for (id, encoded) in pre_contact_objects {
                restored
                    .objects
                    .insert(id, serde_json::from_slice(&encoded).unwrap());
            }
            peer.stamp_airborne_targets_from_locomotor(&[VICTIM]);
            assert_effects(&events, Some(VICTIM), Some(Coord3D::new(0., 0., 0.)));
            owner.stamp_airborne_targets_from_locomotor(&[VICTIM]);
            assert_effects(&events, Some(VICTIM), Some(Coord3D::new(0., 0., 0.)));
            restored.stamp_airborne_targets_from_locomotor(&[VICTIM]);
            assert_effects(&events, Some(VICTIM), Some(Coord3D::new(0., 0., 0.)));
            assert_eq!(peer.session_view.impulse.intensity, 0.);
            assert!(owner.session_view.impulse.intensity > 0.);
            assert_eq!(
                owner.session_view.impulse.intensity, restored.session_view.impulse.intensity,
                "pre-contact restore preserves actual authored effect intensity"
            );
            assert_eq!(
                owner.objects[&VICTIM].health.current,
                restored.objects[&VICTIM].health.current
            );
            assert_eq!(
                owner.objects[&VICTIM].get_position(),
                restored.objects[&VICTIM].get_position()
            );
            assert!(restored.objects[&VICTIM].status.destroyed);
            assert_eq!(foreign_state(), foreign_before);
            // Independent contacts consume independent presentation draws;
            // compare exact offsets only after admitting the same POST-contact
            // saved impulse, rather than requiring newly drawn angles to match.
            let checkpoint = owner.session_view.capture();
            let encoded = serde_json::to_vec(&owner.objects[&VICTIM]).unwrap();
            restored.session_view.admit(checkpoint).unwrap();
            restored
                .objects
                .insert(VICTIM, serde_json::from_slice(&encoded).unwrap());
            let candidate = GameLogic::new();
            drop(candidate);
            peer.reset();
            restored.stamp_airborne_targets_from_locomotor(&[VICTIM]);
            owner.stamp_airborne_targets_from_locomotor(&[VICTIM]);
            assert!(
                events.try_recv().is_err(),
                "restored fired module does not replay"
            );
            for _ in 0..4 {
                assert_eq!(
                    owner.session_view.advance_impulse(),
                    restored.session_view.advance_impulse()
                );
            }
            assert_eq!(
                foreign_state(),
                foreign_before,
                "actual foreign Core tactical view is not a native physics owner"
            );
        },
    );
}

#[test]
fn rest_kill_fx_uses_null_request_source_after_retained_self_fall_source() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "rest_kill_fx_uses_null_request_source_after_retained_self_fall_source",
        || {
            let events = install();
            let mut owner = world(false);
            prepare_fall(&mut owner, false, false);
            owner.stamp_airborne_targets_from_locomotor(&[VICTIM]);
            assert!(owner.objects[&VICTIM].health.current > 70.);
            assert_eq!(owner.objects[&VICTIM].last_damage_source, Some(VICTIM));
            assert!(events.try_recv().is_err());
            owner.frame += 1;
            let object = owner.objects.get_mut(&VICTIM).unwrap();
            object.kill_when_resting_on_ground = true;
            object.movement.velocity = Vec3::ZERO;
            owner.stamp_airborne_targets_from_locomotor(&[VICTIM]);
            assert!(owner.objects[&VICTIM].status.destroyed);
            assert_eq!(
                owner.objects[&VICTIM].last_damage_source,
                Some(VICTIM),
                "retained bookkeeping is separate from this request's NULL dealer"
            );
            assert_effects(&events, None, None);
            assert!(owner.session_view.impulse.intensity > 0.);
        },
    );
}

#[test]
fn shipping_motion_and_shock_destruction_route_actual_receiving_view_effects() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "shipping_motion_and_shock_destruction_route_actual_receiving_view_effects",
        || {
            let events = install();
            let mut owner = world(false);
            let mut peer = world(true);
            prepare_fall(&mut owner, true, true);
            prepare_fall(&mut peer, true, true);
            for world in [&mut peer, &mut owner] {
                world.tick_physics_collisions_all();
                assert!(world.objects[&VICTIM].status.destroyed);
                assert_effects(&events, Some(VICTIM), Some(Coord3D::new(0., 0., 0.)));
            }
            assert_eq!(peer.session_view.impulse.intensity, 0.);
            assert!(owner.session_view.impulse.intensity > 0.);
            let mut stunned = world(false);
            let object = stunned.objects.get_mut(&VICTIM).unwrap();
            object.set_transform_matrix(Mat4::from_scale_rotation_translation(
                Vec3::ONE,
                Quat::from_rotation_z(std::f32::consts::PI),
                Vec3::new(0., 1., 0.),
            ));
            object.movement.velocity = Vec3::new(0., -4., 0.);
            object.shock_stun_frames = 20;
            object.shock_allow_bounce = true;
            object.shock_grounded_once = false;
            object.immune_to_falling_damage = true;
            stunned.tick_shock_stun_all();
            assert!(stunned.objects[&VICTIM].status.destroyed);
            // The inverted kill occurs during bounce; retain its actual current
            // pose rather than assuming terrain placement happened earlier.
            let effects: Vec<_> = events.try_iter().collect();
            assert_eq!(effects.len(), 2);
            match &effects[1] {
                Observation::Die(primary, source) => {
                    assert_eq!(primary.id, VICTIM.0);
                    assert!(
                        source.is_none(),
                        "Object::kill supplies NULL, even with a retained prior source"
                    );
                }
                _ => panic!("authored object death effect missing"),
            }
            assert!(stunned.session_view.impulse.intensity > 0.);
        },
    );
}
