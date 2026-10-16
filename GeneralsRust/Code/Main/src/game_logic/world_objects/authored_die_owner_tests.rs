//! Object.cpp onDie visits authored Die interfaces in order, even after a
//! module queues deletion. These exercise installed native objects and FX.
use crate::game_logic::combat::DamageType;
use crate::game_logic::host_usa_pilot::HostDeathType;
use crate::game_logic::object::DamageHitContext;
use crate::game_logic::{GameLogic, Object, ObjectId, Team, ThingTemplate};
use game_engine::common::ini::ini_fx_list as fx;
use gamelogic::common::{Coord3D, Matrix3D};
use gamelogic::helpers::HostFxObjectPose;
use glam::Vec3;
use std::sync::mpsc::{Receiver, Sender, channel};

const FIRST: &str = "FX_AuthoredDieFirst";
const SECOND: &str = "FX_AuthoredDieSecond";
const CHILD: &str = "FX_AuthoredDieChildTransfer";
const ARMOR: &str = "FX_AuthoredDieVeteranArmor";
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
        panic!("authored OrientToObject must retain object dispatch");
    }
    fn do_fx_obj_host(&self, _: &HostFxObjectPose, secondary: Option<&HostFxObjectPose>) {
        self.1.send((self.0, secondary.map(|p| p.id))).unwrap();
    }
}
fn install() -> Receiver<(&'static str, Option<u32>)> {
    let (sender, receiver) = channel();
    for name in [FIRST, SECOND, CHILD, ARMOR] {
        let mut definition = fx::FXList::new(name.into());
        definition.add_nugget(fx::FXNugget::ViewShake {
            shake_type: fx::CameraShakeType::Normal,
        });
        let mut list = game_client::fx_list::FXList::from_definition(&definition);
        list.add_fx_nugget(Box::new(Observe(name, sender.clone())));
        game_client::fx_list::get_fx_list_store_mut().add_fx_list(name.into(), list);
    }
    receiver
}
fn world(middle: &str) -> (GameLogic, ObjectId, ObjectId) {
    world_with_kind(middle, "INERT IMMOBILE")
}
fn world_with_kind(middle: &str, kind: &str) -> (GameLogic, ObjectId, ObjectId) {
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content(&format!(
        "Object AuthoredDieVictim\nKindOf = {kind}\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = FXListDie First\nDeathFX = {FIRST}\nEnd\n{middle}\nBehavior = FXListDie Last\nDeathFX = {SECOND}\nEnd\nEnd"
    ), "authored-die-owner.ini").unwrap();
    let template = GameLogic::build_template_from_object_definition(
        "AuthoredDieVictim",
        parser.get_definition("AuthoredDieVictim").unwrap(),
        None,
    );
    let mut world = GameLogic::new();
    world.add_player(crate::game_logic::Player::new(
        0,
        Team::Neutral,
        "Neutral",
        false,
    ));
    world.add_player(crate::game_logic::Player::new(1, Team::USA, "Owner", true));
    world.add_player(crate::game_logic::Player::new(
        2,
        Team::China,
        "Source",
        false,
    ));
    world.templates.insert(template.name.clone(), template);
    let victim = world
        .create_object("AuthoredDieVictim", Team::USA, Vec3::ZERO)
        .unwrap();
    world.objects.get_mut(&victim).unwrap().owner_player_id = Some(1);
    let source = ObjectId(90001);
    world.objects.insert(
        source,
        Object::new(ThingTemplate::new("AuthoredSource"), source, Team::China),
    );
    world.objects.get_mut(&source).unwrap().owner_player_id = Some(2);
    {
        let mut shroud = world.world_services.shroud().lock().unwrap();
        shroud.init_shroud_grid(128., 128.);
        shroud.reveal_map_for_player_permanently(1).unwrap();
        shroud.set_host_object_shroud_status(
            1,
            victim.0,
            gamelogic::common::ObjectShroudStatus::Clear,
        );
        shroud.set_host_object_shroud_status(
            1,
            source.0,
            gamelogic::common::ObjectShroudStatus::Clear,
        );
    }
    (world, victim, source)
}
fn hit(world: &mut GameLogic, victim: ObjectId, source: Option<ObjectId>, amount: f32) {
    world
        .apply_owned_damage(
            victim,
            amount,
            source,
            DamageType::Unresistable,
            HostDeathType::Normal,
            None,
            &DamageHitContext::default(),
        )
        .unwrap();
}

#[test]
fn queued_destroy_keeps_later_authored_fx_and_receiving_camera() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "queued_destroy_keeps_later_authored_fx_and_receiving_camera",
        || {
            let events = install();
            let (mut owner, victim, source) = world("Behavior = DestroyDie DeleteTag\nEnd");
            let (peer, peer_id, _) = world("Behavior = DestroyDie DeleteTag\nEnd");
            assert_eq!(peer_id, victim);
            hit(&mut owner, victim, Some(source), 1000.);
            assert_eq!(
                events.try_iter().collect::<Vec<_>>(),
                vec![(FIRST, Some(source.0)), (SECOND, Some(source.0))]
            );
            assert!(
                owner.objects.contains_key(&victim),
                "destroy queues removal until its original phase"
            );
            assert!(owner.objects[&victim].status.destroyed);
            assert!(owner.session_view.impulse.intensity > 0.);
            assert_eq!(peer.session_view.impulse.intensity, 0.);
            assert!(!peer.objects[&peer_id].status.on_die_started);
        },
    );
}

#[test]
fn authored_die_uses_incoming_null_or_self_not_retained_damage_history() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "authored_die_uses_incoming_null_or_self_not_retained_damage_history",
        || {
            let events = install();
            for self_source in [false, true] {
                let (mut owner, victim, source) = world("Behavior = KeepObjectDie KeepTag\nEnd");
                hit(&mut owner, victim, Some(source), 1.);
                assert_eq!(owner.objects[&victim].last_damage_source, Some(source));
                hit(&mut owner, victim, self_source.then_some(victim), 1000.);
                let expected = self_source.then_some(victim.0);
                assert_eq!(
                    events.try_iter().collect::<Vec<_>>(),
                    vec![(FIRST, expected), (SECOND, expected)]
                );
                assert!(
                    owner
                        .objects_to_destroy
                        .iter()
                        .all(|event| event.id != victim),
                    "Keep does not synthesize DestroyDie"
                );
            }
        },
    );
}

#[test]
fn instant_death_does_not_skip_later_die_interfaces() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "instant_death_does_not_skip_later_die_interfaces",
        || {
            let events = install();
            let (mut owner, victim, source) =
                world("Behavior = InstantDeathBehavior InstantTag\nEnd");
            hit(&mut owner, victim, Some(source), 1000.);
            assert_eq!(
                events.try_iter().collect::<Vec<_>>(),
                vec![(FIRST, Some(source.0)), (SECOND, Some(source.0))]
            );
            assert!(owner.objects[&victim].status.destroyed);
        },
    );
}

#[test]
fn slow_initial_fx_is_inline_and_cannot_revoke_prior_destroy_slot() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "slow_initial_fx_is_inline_and_cannot_revoke_prior_destroy_slot",
        || {
            let events = install();
            let (mut owner, victim, source) = world(&format!(
                "Behavior = DestroyDie DeleteTag\nEnd\nBehavior = SlowDeathBehavior SlowTag\nDestructionDelay = 1000\nFX = INITIAL {CHILD}\nEnd"
            ));
            hit(&mut owner, victim, Some(source), 1000.);
            assert_eq!(
                events.try_iter().collect::<Vec<_>>(),
                vec![
                    (FIRST, Some(source.0)),
                    (CHILD, None),
                    (SECOND, Some(source.0))
                ],
                "INITIAL executes before the next authored Die callback"
            );
            assert!(owner.objects[&victim].slow_death.is_some());
            assert!(
                owner.objects[&victim].status.destroyed,
                "Slow cannot undo an earlier destroy request"
            );
            assert_eq!(
                owner
                    .objects_to_destroy
                    .iter()
                    .filter(|event| event.id == victim)
                    .count(),
                1
            );
        },
    );
}

#[test]
fn authored_jet_on_elevated_ground_uses_receiving_ground_effect_before_delete() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "authored_jet_on_elevated_ground_uses_receiving_ground_effect_before_delete",
        || {
            let events = install();
            let (mut owner, victim, source) = world(&format!(
                "Behavior = JetSlowDeathBehavior JetTag\nDeathTypes = NONE\nFXOnGroundDeath = {CHILD}\nFXInitialDeath = {ARMOR}\nEnd"
            ));
            let object = owner.objects.get_mut(&victim).unwrap();
            object.set_position(Vec3::new(0., 100., 0.));
            object.ground_height = 100.;
            assert!(!object.is_significantly_above_terrain());
            hit(&mut owner, victim, Some(source), 1000.);
            assert_eq!(
                events.try_iter().collect::<Vec<_>>(),
                vec![
                    (FIRST, Some(source.0)),
                    (CHILD, None),
                    (SECOND, Some(source.0))
                ],
                "Jet ground override bypasses the Slow mux and name heuristics"
            );
            assert!(
                owner.objects[&victim]
                    .jet_slow_death
                    .as_ref()
                    .unwrap()
                    .started_on_ground
            );
            assert!(owner.objects[&victim].status.destroyed);
            assert!(owner.objects.contains_key(&victim));
        },
    );
}

#[test]
fn retained_authored_die_receipts_survive_encoded_world_restore() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "retained_authored_die_receipts_survive_encoded_world_restore",
        || {
            let events = install();
            let modules = "Behavior = KeepObjectDie KeepTag\nEnd";
            let (mut owner, victim, source) = world(modules);
            hit(&mut owner, victim, Some(source), 1000.);
            assert_eq!(events.try_iter().count(), 2);
            let builder = crate::save_load::SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&owner).unwrap();
            let bytes = bincode_legacy::serialize(&snapshot).unwrap();
            let decoded =
                crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
            let (mut restored, restored_id, _) = world(modules);
            assert_eq!(restored_id, victim);
            builder
                .restore_from_snapshot(&decoded, &mut restored)
                .unwrap();
            assert_eq!(
                restored.objects[&victim].native_die_runtime.completed,
                owner.objects[&victim].native_die_runtime.completed
            );
            assert!(restored.objects[&victim].status.on_die_started);
            for world in [&mut owner, &mut restored] {
                world.mark_object_for_destruction(victim, Some(Team::China));
                assert!(
                    events.try_recv().is_err(),
                    "saved onDie must not replay effects"
                );
            }
        },
    );
}

#[test]
fn bridge_repair_starts_a_new_authored_die_episode() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "bridge_repair_starts_a_new_authored_die_episode",
        || {
            let events = install();
            let (mut owner, victim, source) = world_with_kind(
                "Behavior = BridgeBehavior BridgeTag\nEnd",
                "BRIDGE STRUCTURE IMMOBILE",
            );
            hit(&mut owner, victim, Some(source), 1000.);
            assert_eq!(
                events.try_iter().collect::<Vec<_>>(),
                vec![(FIRST, Some(source.0)), (SECOND, Some(source.0))]
            );
            assert!(owner.objects[&victim].status.on_die_started);
            assert!(
                !owner.objects[&victim]
                    .native_die_runtime
                    .completed
                    .is_empty()
            );
            assert!(
                owner
                    .objects_to_destroy
                    .iter()
                    .all(|event| event.id != victim)
            );
            owner.apply_owned_healing(victim, 100., None).unwrap();
            assert_eq!(owner.objects[&victim].health.current, 100.);
            assert!(!owner.objects[&victim].status.on_die_started);
            assert!(!owner.objects[&victim].status.effectively_dead);
            assert!(
                owner.objects[&victim]
                    .native_die_runtime
                    .completed
                    .is_empty()
            );
            assert!(
                events.try_recv().is_err(),
                "repair does not replay Die effects"
            );
            hit(&mut owner, victim, Some(source), 1000.);
            assert_eq!(
                events.try_iter().collect::<Vec<_>>(),
                vec![(FIRST, Some(source.0)), (SECOND, Some(source.0))],
                "the next actual death delivers each authored callback again"
            );
            assert!(
                !owner.objects[&victim]
                    .native_die_runtime
                    .completed
                    .is_empty()
            );
        },
    );
}

#[test]
fn duplicate_create_slots_interleave_with_fx_and_transfer_previous_body_history() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "duplicate_create_slots_interleave_with_fx_and_transfer_previous_body_history",
        || {
            use game_engine::common::ini::ini_damage_fx as rules;
            use gamelogic::object_creation_list::{
                GenericObjectCreationNugget, store::ObjectCreationList,
            };
            let events = install();
            let mut ocl = ObjectCreationList::new();
            ocl.add_nugget(std::sync::Arc::new(GenericObjectCreationNugget {
                names: vec!["AuthoredDieChild".into()],
                name_are_objects: true,
                ..Default::default()
            }));
            gamelogic::helpers::TheObjectCreationListStore::register_object_creation_list(
                "OCL_AuthoredDieChild",
                ocl,
            );
            let mut damage_fx = rules::DamageFX::new();
            damage_fx.set_major_minor_fx(
                rules::DamageType::Unresistable,
                Some(CHILD.into()),
                Some(CHILD.into()),
                0.,
            );
            rules::init_global_damage_fx_store();
            rules::get_damage_fx_store_mut()
                .unwrap()
                .add_damage_fx("DamageFX_AuthoredDieChild".into(), damage_fx);
            let (mut owner, victim, source) = world(&format!(
                "Behavior = CreateObjectDie CreateFirst\nCreationList = OCL_AuthoredDieChild\nTransferPreviousHealth = Yes\nEnd\nBehavior = FXListDie Between\nDeathFX = {SECOND}\nEnd\nBehavior = CreateObjectDie CreateLast\nCreationList = OCL_AuthoredDieChild\nTransferPreviousHealth = Yes\nEnd"
            ));
            let mut child = ThingTemplate::new("AuthoredDieChild");
            child.set_health(100.);
            child.armor_sets.push(crate::game_logic::HostArmorSet {
                conditions: 0,
                armor: None,
                damage_fx: Some("DamageFX_AuthoredDieChild".into()),
            });
            owner.templates.insert(child.name.clone(), child);
            hit(&mut owner, victim, Some(source), 1.);
            hit(&mut owner, victim, None, 1000.);
            assert_eq!(
                events.try_iter().collect::<Vec<_>>(),
                vec![
                    (FIRST, None),
                    (CHILD, Some(source.0)),
                    (SECOND, None),
                    (CHILD, Some(source.0)),
                    (SECOND, None),
                ],
                "OCL transfer uses historical source; surrounding Die FX uses incoming null"
            );
            let children: Vec<_> = owner
                .objects
                .values()
                .filter(|o| o.template_name == "AuthoredDieChild")
                .collect();
            assert_eq!(children.len(), 2, "each authored Create slot executes once");
            assert!(children.iter().all(|o| o.health.current == 99.));
        },
    );
}

#[test]
fn actual_death_weapon_runs_after_kill_credit_before_outer_armor_fx() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_death_weapon_runs_after_kill_credit_before_outer_armor_fx",
        || {
            use crate::game_logic::{HostArmorSet, Player, VeterancyLevel};
            use game_engine::common::ini::ini_damage_fx as rules;
            let events = install();
            let mut definition = rules::DamageFX::new();
            definition.set_major_minor_fx(
                rules::DamageType::Unresistable,
                Some(SECOND.into()),
                Some(SECOND.into()),
                0.,
            );
            definition.set_major_minor_fx_at_level(
                rules::DamageType::Unresistable,
                1,
                Some(ARMOR.into()),
                Some(ARMOR.into()),
                0.,
            );
            rules::init_global_damage_fx_store();
            rules::get_damage_fx_store_mut()
                .unwrap()
                .add_damage_fx("DamageFX_AuthoredDieRank".into(), definition);
            for blast in [110, 200] {
                let name = format!("AuthoredDieBlast{blast}");
                let mut rows = std::collections::HashMap::new();
                assert_eq!(
                    crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
                        &mut rows,
                        &format!(
                            "Weapon {name}\nPrimaryDamage = {blast}\nPrimaryDamageRadius = 100\nDamageType = EXPLOSION\nRadiusDamageAffects = ALLIES ENEMIES NEUTRALS\nProjectileObject = NONE\nClipSize = 1\nClipReloadTime = 0\nDelayBetweenShots = 0\nEnd"
                        ),
                        crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal
                    ),
                    1
                );
                let (mut owner, victim, source) = world(&format!(
                    "Behavior = FireWeaponWhenDeadBehavior BlastTag\nStartsActive = Yes\nDeathWeapon = {name}\nEnd\nBehavior = KeepObjectDie KeepTag\nEnd"
                ));
                owner
                    .admit_weapon_content_snapshot(rows.into_iter().collect(), &[])
                    .unwrap();
                for (id, team) in [(1, Team::USA), (2, Team::China)] {
                    let mut player = Player::new(id, team, "AuthoredDeathOwner", id == 1);
                    player.alliance_team = id as i32;
                    player.is_alive = true;
                    owner.add_player(player);
                }
                {
                    let object = owner.objects.get_mut(&victim).unwrap();
                    object.owner_player_id = Some(1);
                    object.template_mut().experience_value = 100.;
                    object.template_mut().experience_values = [100.; 4];
                    object.template_mut().armor_sets.push(HostArmorSet {
                        conditions: 0,
                        armor: None,
                        damage_fx: Some("DamageFX_AuthoredDieRank".into()),
                    });
                }
                {
                    let object = owner.objects.get_mut(&source).unwrap();
                    object.owner_player_id = Some(2);
                    object.template_mut().is_trainable = true;
                    object.template_mut().veterancy_xp_thresholds = [1., 100000., 200000.];
                }
                hit(&mut owner, victim, Some(source), 1000.);
                assert_eq!(
                    owner.objects[&source].experience.level,
                    VeterancyLevel::Veteran
                );
                assert_eq!(owner.objects[&source].health.maximum, 100.0_f32 * 1.2);
                assert_eq!(
                    owner.objects[&source].health.current,
                    (100.0_f32 * 1.2 - blast as f32).max(0.),
                    "kill credit promotes before the synchronous authored blast"
                );
                assert!(
                    owner.objects.contains_key(&source),
                    "dead source remains installed until physical disposal"
                );
                assert_eq!(owner.objects[&source].status.on_die_started, blast == 200);
                assert_eq!(owner.objects[&source].status.destroyed, blast == 200);
                assert_eq!(
                    owner
                        .objects_to_destroy
                        .iter()
                        .filter(|event| event.id == source)
                        .count(),
                    usize::from(blast == 200),
                    "the dead source is queued once while remaining visible to outer ArmorFX"
                );
                assert_eq!(
                    events.try_iter().collect::<Vec<_>>(),
                    vec![
                        (FIRST, Some(source.0)),
                        (SECOND, Some(source.0)),
                        (ARMOR, Some(source.0))
                    ],
                    "outer Armor observes the newly promoted still-resident source after death callbacks"
                );
            }
        },
    );
}
