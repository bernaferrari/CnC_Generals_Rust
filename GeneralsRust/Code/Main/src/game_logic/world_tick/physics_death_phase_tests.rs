//! CPP PhysicsUpdate1475/1742 and SquishCollide: attemptDamage completes
//! score/onDie/Armor before collision processing resumes on installed objects.
use crate::game_logic::{
    GameLogic, HostArmorSet, KindOf, Object, ObjectId, Player, Team, ThingTemplate, VeterancyLevel,
};
use game_engine::common::ini::{ini_damage_fx as damage, ini_fx_list as fx};
use gamelogic::common::{Coord3D, Matrix3D, ObjectShroudStatus};
use gamelogic::helpers::HostFxObjectPose;
use glam::Vec3;
use std::sync::mpsc::{Receiver, Sender, channel};

const ZERO: &str = "FX_CrushPhaseZero";
const DIE: &str = "FX_CrushPhaseDie";
const NORMAL: &str = "FX_CrushPhaseNormal";
const VETERAN: &str = "FX_CrushPhaseVeteran";
const CRUSHER: ObjectId = ObjectId(98001);

struct Observe(&'static str, Sender<(&'static str, Option<u32>)>);
impl game_client::fx_list::FXNugget for Observe {
    fn do_fx_pos(
        &self,
        _: Option<&Coord3D>,
        _: Option<&Matrix3D>,
        _: f32,
        _: Option<&Coord3D>,
        _: f32,
    ) {
        panic!("crush Die/Armor must retain object-form FX");
    }
    fn do_fx_obj_host(&self, _: &HostFxObjectPose, secondary: Option<&HostFxObjectPose>) {
        self.1
            .send((self.0, secondary.map(|pose| pose.id)))
            .unwrap();
    }
}
fn install() -> Receiver<(&'static str, Option<u32>)> {
    let (send, receive) = channel();
    for name in [ZERO, DIE, NORMAL, VETERAN] {
        let mut definition = fx::FXList::new(name.into());
        definition.add_nugget(fx::FXNugget::ViewShake {
            shake_type: fx::CameraShakeType::Normal,
        });
        let mut list = game_client::fx_list::FXList::from_definition(&definition);
        list.add_fx_nugget(Box::new(Observe(name, send.clone())));
        game_client::fx_list::get_fx_list_store_mut().add_fx_list(name.into(), list);
    }
    let mut rules = damage::DamageFX::new();
    rules.set_major_minor_fx(
        damage::DamageType::Crush,
        Some(NORMAL.into()),
        Some(ZERO.into()),
        1.,
    );
    rules.set_major_minor_fx_at_level(
        damage::DamageType::Crush,
        1,
        Some(VETERAN.into()),
        Some(ZERO.into()),
        1.,
    );
    damage::init_global_damage_fx_store();
    damage::get_damage_fx_store_mut()
        .unwrap()
        .add_damage_fx("DamageFX_CrushPhase".into(), rules);
    receive
}
fn world(squish: bool) -> (GameLogic, ObjectId) {
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content(&format!("Object CrushPhaseVictim\nKindOf = INFANTRY SCORE\nGeometry = CYLINDER\nGeometryMajorRadius = 4\nGeometryMinorRadius = 4\nGeometryHeight = 8\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = FXListDie FxTag\nDeathFX = {DIE}\nEnd\nBehavior = CrushDie CrushTag\nEnd\nBehavior = KeepObjectDie KeepTag\nEnd\nEnd"), "crush-phase.ini").unwrap();
    let mut template = GameLogic::build_template_from_object_definition(
        "CrushPhaseVictim",
        parser.get_definition("CrushPhaseVictim").unwrap(),
        None,
    );
    template.experience_value = 100.;
    template.experience_values = [100.; 4];
    template.armor_sets.push(HostArmorSet {
        conditions: 0,
        armor: None,
        damage_fx: Some("DamageFX_CrushPhase".into()),
    });
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::Neutral, "Neutral", false));
    for (id, team) in [(1, Team::USA), (2, Team::China)] {
        let mut player = Player::new(id, team, "CrushPhase", id == 1);
        player.alliance_team = id as i32;
        world.add_player(player);
    }
    world.templates.insert(template.name.clone(), template);
    let victim = world
        .create_object_for_player("CrushPhaseVictim", 1, Vec3::ZERO)
        .unwrap();
    let object = world.objects.get_mut(&victim).unwrap();
    object.crushable_level = 0;
    object.selection_radius = 4.;
    object.has_squish_collide = squish;
    let mut tank = ThingTemplate::new("CrushPhaseSource");
    tank.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Score)
        .set_health(100.);
    tank.is_trainable = true;
    tank.veterancy_xp_thresholds = [1., 100000., 200000.];
    tank.geometry_info = crate::game_logic::HostGeometryInfo {
        geom_type: crate::game_logic::HostGeometryType::Cylinder,
        is_small: true,
        height: 4.,
        major_radius: 2.,
        minor_radius: 2.,
        authored: true,
    };
    world.templates.insert(tank.name.clone(), tank.clone());
    let mut crusher = Object::new(tank, CRUSHER, Team::China);
    crusher.owner_player_id = Some(2);
    crusher.crusher_level = 2;
    crusher.set_position(Vec3::new(-20., 0., 0.));
    crusher.set_orientation(0.);
    crusher.movement.velocity = Vec3::X * 5.;
    world.objects.insert(CRUSHER, crusher);
    let mut shroud = world.world_services.shroud().lock().unwrap();
    shroud.init_shroud_grid(128., 128.);
    shroud.reveal_map_for_player_permanently(1).unwrap();
    for id in [CRUSHER, victim] {
        shroud.set_host_object_shroud_status(1, id.0, ObjectShroudStatus::Clear);
    }
    drop(shroud);
    (world, victim)
}

#[test]
fn installed_overlap_completes_death_and_promotion_before_armor_in_both_crush_paths() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "installed_overlap_completes_death_and_promotion_before_armor_in_both_crush_paths",
        || {
            let events = install();
            for squish in [false, true] {
                let (mut owner, victim) = world(squish);
                let (peer, peer_victim) = world(squish);
                assert_eq!(victim, peer_victim);
                assert!(owner.apply_overlap_crush_check(CRUSHER, victim, false));
                assert_eq!(owner.objects[&victim].health.current, 100.);
                assert!(!owner.objects[&victim].is_undetected_defector());
                assert_eq!(
                    events.try_iter().collect::<Vec<_>>(),
                    vec![],
                    "CPP DamageFX::getDamageFXList suppresses zero-amount nuggets"
                );
                assert_eq!(
                    owner.objects[&victim].last_damage_fx_done,
                    Some(crate::game_logic::combat::DamageType::Crush),
                    "zero contact still commits Armor throttle bookkeeping before geometry"
                );
                owner
                    .objects
                    .get_mut(&CRUSHER)
                    .unwrap()
                    .set_position(Vec3::new(if squish { -0.5 } else { 2.2 }, 0., 0.));
                assert!(owner.apply_overlap_crush_check(CRUSHER, victim, false));
                assert_eq!(
                    events.try_iter().collect::<Vec<_>>(),
                    vec![(DIE, Some(CRUSHER.0)), (VETERAN, Some(CRUSHER.0))],
                    "actual world onDie and kill promotion must complete before outer Armor"
                );
                assert!(owner.objects[&victim].status.on_die_started);
                assert!(
                    owner.objects[&victim]
                        .native_die_runtime
                        .completed
                        .contains(&1)
                );
                assert_eq!(
                    owner.objects[&CRUSHER].experience.level,
                    VeterancyLevel::Veteran
                );
                assert_eq!(owner.objects[&CRUSHER].experience.current, 100.);
                assert_eq!(
                    owner.objects.len(),
                    2,
                    "callbacks run with both identities installed"
                );
                assert!(owner.session_view.impulse.intensity > 0.);
                assert_eq!(peer.session_view.impulse.intensity, 0.);
                assert_eq!(peer.objects[&victim].health.current, 100.);
                assert!(!peer.objects[&victim].status.on_die_started);
                let builder = crate::save_load::snapshot::SnapshotBuilder::new();
                let saved = builder.create_world_snapshot(&owner).unwrap();
                let bytes = bincode_legacy::serialize(&saved).unwrap();
                let decoded =
                    crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
                // C++ load admits the same map/player/team definitions before
                // applying a saved roster. A bare world has no prototypes.
                let (mut restored, restored_victim) = world(squish);
                assert_eq!(restored_victim, victim);
                builder
                    .restore_from_snapshot(&decoded, &mut restored)
                    .unwrap();
                assert!(restored.objects[&victim].status.on_die_started);
                assert_eq!(
                    restored.objects[&victim].native_die_runtime.completed,
                    owner.objects[&victim].native_die_runtime.completed
                );
                assert!(
                    events.try_recv().is_err(),
                    "restoring completed death never replays its FX"
                );
            }
        },
    );
}

#[test]
fn initial_contact_continuation_rereads_crush_flags_and_current_geometry() {
    use crate::game_logic::object::{OverlapCrushResume, OverlapCrushStep};
    for callback_crushed_both in [false, true] {
        let (mut world, victim) = world(false);
        let [Some(crusher), Some(crushee)] = world.objects.get_disjoint_mut([&CRUSHER, &victim])
        else {
            panic!("admitted pair")
        };
        let OverlapCrushStep::Damage { request, resume } =
            crusher.step_overlap_crush(crushee, false, OverlapCrushResume::Start)
        else {
            panic!("initial contact must yield its zero hit")
        };
        assert_eq!(request.amount, 0.);
        assert_eq!(request.source, Some(CRUSHER));
        assert!(matches!(resume, OverlapCrushResume::AfterInitialContact));
        // Model changes made during the completed synchronous zero-hit callback.
        // The continuation may carry phase identity, never these future reads.
        crusher.set_position(Vec3::new(2.2, 0., 0.));
        crushee.front_crushed = callback_crushed_both;
        crushee.back_crushed = callback_crushed_both;
        let next = crusher.step_overlap_crush(crushee, false, resume);
        if callback_crushed_both {
            assert!(matches!(next, OverlapCrushStep::Complete(true)));
        } else {
            let OverlapCrushStep::Damage { request, .. } = next else {
                panic!("fresh position passes the crush point")
            };
            assert!(request.amount > 100.);
            assert_eq!(request.victim, victim);
        }
    }
    let (mut world, victim) = world(true);
    let [Some(crusher), Some(crushee)] = world.objects.get_disjoint_mut([&CRUSHER, &victim]) else {
        panic!("admitted squish pair")
    };
    crusher.set_position(Vec3::new(-0.5, 0., 0.));
    let OverlapCrushStep::Damage { resume, .. } =
        crusher.step_overlap_crush(crushee, false, OverlapCrushResume::Start)
    else {
        panic!("eligible Squish must yield its damage request")
    };
    assert!(matches!(resume, OverlapCrushResume::AfterSquish { .. }));
    // A completed damage callback can change cover. CPP106 clears it after
    // attemptDamage returns, even when the Body rejected that damage.
    crushee.begin_undetected_defection(0, 1000, true);
    assert!(crushee.is_undetected_defector());
    assert!(matches!(
        crusher.step_overlap_crush(crushee, false, resume),
        OverlapCrushStep::Complete(true)
    ));
    assert!(!crushee.is_undetected_defector());
    assert_eq!(crushee.health.current, 100.);
}
