//! ActiveBody309–315 and DamageFX61–93 through the real native damage owner.
//! These witnesses cover receiving FX and continuation, not full GPU loading.
use crate::game_logic::combat::DamageType;
use crate::game_logic::host_usa_pilot::HostDeathType;
use crate::game_logic::object::DamageHitContext;
use crate::game_logic::{
    GameLogic, HostArmorSet, Object, ObjectId, Player, Team, ThingTemplate, VeterancyLevel,
};
use game_engine::common::ini::{ini_damage_fx as rules, ini_fx_list as fx};
use gamelogic::common::{Coord3D, Matrix3D, ObjectShroudStatus};
use gamelogic::helpers::HostFxObjectPose;
use glam::Vec3;
use std::sync::mpsc::{Receiver, Sender, channel};

const VICTIM: ObjectId = ObjectId(9811);
const SOURCE: ObjectId = ObjectId(9812);
const DEFINITION: &str = "DamageFX_ReceivingArmorOwner";
const MINOR: &str = "FX_ReceivingArmorMinor";
const MAJOR: &str = "FX_ReceivingArmorMajor";
const VETERAN: &str = "FX_ReceivingArmorVeteran";

#[derive(Debug)]
struct Observed(&'static str, HostFxObjectPose, Option<HostFxObjectPose>);
struct Observe(&'static str, Sender<Observed>);
impl game_client::fx_list::FXNugget for Observe {
    fn do_fx_pos(
        &self,
        _: Option<&Coord3D>,
        _: Option<&Matrix3D>,
        _: f32,
        _: Option<&Coord3D>,
        _: f32,
    ) {
        panic!("DamageFX must retain object primary/secondary dispatch");
    }
    fn do_fx_obj_host(&self, primary: &HostFxObjectPose, secondary: Option<&HostFxObjectPose>) {
        self.1
            .send(Observed(self.0, *primary, secondary.copied()))
            .unwrap();
    }
}
fn install() -> Receiver<Observed> {
    let (sender, receiver) = channel();
    for name in [MINOR, MAJOR, VETERAN] {
        let mut definition = fx::FXList::new(name.into());
        definition.add_nugget(fx::FXNugget::ViewShake {
            shake_type: fx::CameraShakeType::Normal,
        });
        let mut list = game_client::fx_list::FXList::from_definition(&definition);
        list.add_fx_nugget(Box::new(Observe(name, sender.clone())));
        game_client::fx_list::get_fx_list_store_mut().add_fx_list(name.into(), list);
    }
    let mut definition = rules::DamageFX::new();
    for dt in [
        rules::DamageType::SmallArms,
        rules::DamageType::Explosion,
        rules::DamageType::Healing,
        rules::DamageType::Unresistable,
        rules::DamageType::Status,
        rules::DamageType::Falling,
    ] {
        definition.set_major_minor_fx(dt, Some(MAJOR.into()), Some(MINOR.into()), 25.);
        definition.set_major_minor_fx_at_level(
            dt,
            1,
            Some(VETERAN.into()),
            Some(VETERAN.into()),
            25.,
        );
    }
    let mut ini = game_engine::common::ini::INI::new();
    rules::parse_time(&mut ini, &mut definition, &["Default", "1000"]).unwrap();
    rules::parse_veterancy_time(&mut ini, &mut definition, &["VETERAN", "Default", "2000"])
        .unwrap();
    rules::init_global_damage_fx_store();
    rules::get_damage_fx_store_mut()
        .unwrap()
        .add_damage_fx(DEFINITION.into(), definition);
    receiver
}
fn world() -> GameLogic {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::Neutral, "Neutral", false));
    world.add_player(Player::new(1, Team::USA, "Owner", true));
    world.add_player(Player::new(2, Team::China, "Attacker", false));
    let mut template = ThingTemplate::new("ArmorOwnerVictim");
    template.set_health(1000.);
    template.armor_sets.push(HostArmorSet {
        conditions: 0,
        armor: None,
        damage_fx: Some(DEFINITION.into()),
    });
    let mut victim = Object::new(template, VICTIM, Team::USA);
    victim.owner_player_id = Some(1);
    victim.set_position(Vec3::new(7., 0., 11.));
    world.objects.insert(VICTIM, victim);
    let mut source = Object::new(ThingTemplate::new("ArmorOwnerSource"), SOURCE, Team::China);
    source.owner_player_id = Some(2);
    source.set_position(Vec3::new(17., 0., 23.));
    world.objects.insert(SOURCE, source);
    reveal(&world);
    world
}
fn reveal(world: &GameLogic) {
    let mut shroud = world.world_services.shroud().lock().unwrap();
    shroud.init_shroud_grid(128., 128.);
    shroud.reveal_map_for_player_permanently(1).unwrap();
    shroud.set_host_object_shroud_status(1, VICTIM.0, ObjectShroudStatus::Clear);
    shroud.set_host_object_shroud_status(1, SOURCE.0, ObjectShroudStatus::Shrouded);
}
fn hit(world: &mut GameLogic, amount: f32, source: Option<ObjectId>, dt: DamageType) {
    world
        .apply_owned_damage(
            VICTIM,
            amount,
            source,
            dt,
            HostDeathType::Normal,
            None,
            &DamageHitContext::default(),
        )
        .unwrap();
}
fn one(events: &Receiver<Observed>, expected: &'static str, source: Option<ObjectId>) {
    let event = events
        .try_recv()
        .expect("actual authored Armor FX must execute inline");
    assert_eq!(event.0, expected);
    assert_eq!(event.1.id, VICTIM.0);
    assert_eq!(event.1.position, Coord3D::new(7., 11., 0.));
    assert_eq!(event.2.map(|p| ObjectId(p.id)), source);
    if source == Some(SOURCE) {
        assert_eq!(event.2.unwrap().position, Coord3D::new(17., 23., 0.));
    }
    assert!(
        events.try_recv().is_err(),
        "selected list must execute once"
    );
}
fn foreign_state() -> (u32, u32, u32, u32, u32, u32) {
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
}

#[test]
fn native_armor_fx_uses_receiving_camera_source_rank_and_current_source_pose() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "native_armor_fx_uses_receiving_camera_source_rank_and_current_source_pose",
        || {
            let events = install();
            game_client::display::view::with_tactical_view(|view| {
                use game_client::display::view::{CameraShakeType, Point3};
                let at = Point3::new(-100000., -200000., 9.);
                view.set_position(&at);
                view.reset_camera_shake();
                let mut rules = game_engine::common::global_data::GlobalData::default();
                rules.shake_normal_intensity = 1.25;
                rules.max_shake_intensity = 10.;
                rules.max_shake_range = 100.;
                view.shake_with_services(&at, CameraShakeType::Normal, &rules, &mut |_, _| 0.75);
                view.tick_impulse_shake();
            });
            let foreign = foreign_state();
            let mut owner = world();
            let mut peer = world();
            peer.session_view.target = Vec3::splat(100000.);
            owner.objects.get_mut(&SOURCE).unwrap().experience.level = VeterancyLevel::Veteran;
            owner.frame = 123;
            hit(&mut owner, 5., Some(SOURCE), DamageType::Bullet);
            one(&events, VETERAN, Some(SOURCE));
            assert_eq!(owner.objects[&VICTIM].next_damage_fx_time, 183);
            assert!(
                owner.session_view.impulse.intensity > 0.,
                "ambient ArmorFX leaves receiving owner untouched"
            );
            assert_eq!(peer.session_view.impulse.intensity, 0.);
            assert_eq!(foreign_state(), foreign);
            // Change the source rank, not the victim rank, after the throttle ends.
            owner.frame = 183;
            owner.objects.get_mut(&SOURCE).unwrap().experience.level = VeterancyLevel::Rookie;
            owner.objects.get_mut(&VICTIM).unwrap().experience.level = VeterancyLevel::Veteran;
            hit(&mut owner, 5., Some(SOURCE), DamageType::Bullet);
            one(&events, MINOR, Some(SOURCE));
            owner.frame = 213;
            hit(&mut owner, 30., Some(SOURCE), DamageType::Bullet);
            one(&events, MAJOR, Some(SOURCE));
            peer.reset();
            assert_eq!(foreign_state(), foreign);
        },
    );
}

#[test]
fn native_armor_zero_throttle_and_healing_continue_after_object_view_restore() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "native_armor_zero_throttle_and_healing_continue_after_object_view_restore",
        || {
            let events = install();
            let mut owner = world();
            owner.frame = 10;
            hit(&mut owner, 0., None, DamageType::Bullet);
            assert!(events.try_recv().is_err());
            assert_eq!(owner.objects[&VICTIM].next_damage_fx_time, 40);
            hit(&mut owner, 5., None, DamageType::Bullet);
            assert!(
                events.try_recv().is_err(),
                "zero amount still commits throttle"
            );
            let before = owner.session_view.capture();
            let objects =
                [VICTIM, SOURCE].map(|id| (id, serde_json::to_vec(&owner.objects[&id]).unwrap()));
            let mut restored = world();
            restored.frame = 10;
            restored.session_view.admit(before).unwrap();
            for (id, bytes) in objects {
                restored
                    .objects
                    .insert(id, serde_json::from_slice(&bytes).unwrap());
            }
            for world in [&mut owner, &mut restored] {
                world.frame = 39;
                hit(world, 5., None, DamageType::Bullet);
                assert!(events.try_recv().is_err());
                world.frame = 40;
                hit(world, 5., None, DamageType::Bullet);
                one(&events, MINOR, None);
                // A different type bypasses the previous type's deadline.
                hit(world, 30., None, DamageType::Explosive);
                one(&events, MAJOR, None);
                world
                    .apply_owned_healing(VICTIM, 10., Some(SOURCE))
                    .unwrap();
                one(&events, MINOR, Some(SOURCE));
                assert_eq!(world.objects[&VICTIM].next_damage_fx_time, 70);
            }
            assert_eq!(
                owner.objects[&VICTIM].health.current,
                restored.objects[&VICTIM].health.current
            );
            assert_eq!(
                owner.session_view.impulse.intensity,
                restored.session_view.impulse.intensity
            );
            let checkpoint = owner.session_view.capture();
            restored.session_view.admit(checkpoint).unwrap();
            for _ in 0..4 {
                owner.session_view.advance_impulse();
                restored.session_view.advance_impulse();
                assert_eq!(
                    owner.session_view.capture(),
                    restored.session_view.capture()
                );
            }
            assert!(
                events.try_recv().is_err(),
                "admission and visual continuation never replay Armor FX"
            );
        },
    );
}

#[test]
fn native_armor_early_status_missing_source_and_capped_healing_keep_original_amounts() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "native_armor_early_status_missing_source_and_capped_healing_keep_original_amounts",
        || {
            let events = install();
            let mut owner = world();
            owner.objects.get_mut(&VICTIM).unwrap().last_damage_source = Some(SOURCE);
            owner.frame = 17;
            owner.objects.get_mut(&SOURCE).unwrap().experience.level = VeterancyLevel::Veteran;
            hit(&mut owner, 5., Some(SOURCE), DamageType::Status);
            one(&events, VETERAN, Some(SOURCE));
            assert_eq!(owner.objects[&VICTIM].next_damage_fx_time, 77);
            assert_eq!(owner.objects[&VICTIM].health.current, 1000.);
            owner.frame = 77;
            let frozen =
                DamageHitContext::new(owner.objects.get(&SOURCE), None, DamageType::Status);
            owner
                .apply_owned_damage(
                    VICTIM,
                    30.,
                    Some(ObjectId(99999)),
                    DamageType::Status,
                    HostDeathType::Normal,
                    None,
                    &frozen,
                )
                .unwrap();
            one(&events, MAJOR, None);
            assert_eq!(owner.objects[&VICTIM].health.current, 1000.);
            assert!(owner.session_view.impulse.intensity > 0.);
            // Healing FX uses the adjusted input, even when only 5HP can fit.
            owner.objects.get_mut(&VICTIM).unwrap().health.current = 995.;
            owner
                .apply_owned_healing(VICTIM, 30., Some(VICTIM))
                .unwrap();
            one(&events, MAJOR, Some(VICTIM));
            assert_eq!(owner.objects[&VICTIM].health.current, 1000.);
        },
    );
}

#[test]
fn native_armor_selection_observes_kill_credit_promotion_before_fx() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "native_armor_selection_observes_kill_credit_promotion_before_fx",
        || {
            let events = install();
            let mut owner = world();
            owner.players.get_mut(&1).unwrap().alliance_team = 1;
            owner.players.get_mut(&2).unwrap().alliance_team = 2;
            assert_eq!(
                owner.player_relationship(2, 1),
                gamelogic::common::Relationship::Enemies
            );
            let source = owner.objects.get_mut(&SOURCE).unwrap();
            source.template_mut().is_trainable = true;
            source.template_mut().veterancy_xp_thresholds = [1., 100000., 200000.];
            let victim = owner.objects.get_mut(&VICTIM).unwrap();
            victim.template_mut().experience_value = 100.;
            victim.template_mut().experience_values = [100.; 4];
            assert_eq!(
                owner.objects[&SOURCE].experience.level,
                VeterancyLevel::Rookie
            );
            owner.frame = 77;
            hit(&mut owner, 2000., Some(SOURCE), DamageType::Bullet);
            one(&events, VETERAN, Some(SOURCE));
            assert_eq!(
                owner.objects[&SOURCE].experience.level,
                VeterancyLevel::Veteran
            );
            assert!(owner.objects[&VICTIM].kill_experience_awarded);
            assert!(owner.objects[&VICTIM].status.on_die_started);
            assert_eq!(owner.objects[&VICTIM].next_damage_fx_time, 137);
            assert!(
                owner.objects.contains_key(&VICTIM),
                "onDie does not physically remove the receiver"
            );
        },
    );
}

#[test]
fn native_residual_and_physics_armor_tails_use_their_receiving_services() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "native_residual_and_physics_armor_tails_use_their_receiving_services",
        || {
            let events = install();
            let mut owner = world();
            owner.objects.get_mut(&SOURCE).unwrap().experience.level = VeterancyLevel::Veteran;
            owner.frame = 100;
            assert!(!owner.apply_residual_damage_owned(
                VICTIM,
                5.,
                Some(SOURCE),
                DamageType::Bullet,
                HostDeathType::Normal,
                None,
            ));
            one(&events, VETERAN, Some(SOURCE));
            assert!(owner.session_view.impulse.intensity > 0.);
            let mut falling = world();
            falling.frame = 101;
            let victim = falling.objects.get_mut(&VICTIM).unwrap();
            victim.experience.level = VeterancyLevel::Veteran;
            victim.set_position(Vec3::new(7., -1., 11.));
            victim.was_airborne_last_frame = true;
            victim.immune_to_falling_damage = false;
            victim.movement.velocity = Vec3::new(0., -4., 0.);
            victim.physics_mass = 1.;
            victim.fall_height_damage_factor = 1.;
            victim.stick_to_ground = false;
            victim.allow_to_fall = true;
            falling.stamp_airborne_targets_from_locomotor(&[VICTIM]);
            one(&events, VETERAN, Some(VICTIM));
            assert!(falling.objects[&VICTIM].health.current < 1000.);
            assert!(falling.session_view.impulse.intensity > 0.);
        },
    );
}
