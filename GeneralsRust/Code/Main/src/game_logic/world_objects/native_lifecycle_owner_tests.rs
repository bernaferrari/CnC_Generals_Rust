//! Actual authored updates on the receiving Object, without ambient registries.
use super::super::*;
use crate::game_logic::native_lifecycle::NativeLifecycleState;

fn authored(name: &str, modules: &str, frame: u32, position: Vec3) -> (GameLogic, ObjectId) {
    authored_with_extra(name, modules, frame, position, "")
}

fn authored_with_extra(
    name: &str,
    modules: &str,
    frame: u32,
    position: Vec3,
    extra: &str,
) -> (GameLogic, ObjectId) {
    let source = format!(
        "Object {name}\nKindOf = IMMOBILE INERT NO_COLLIDE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\n{modules}\nEnd\n{extra}"
    );
    let content = crate::assets::ReceivingObjectContent::from_sources([(
        "native-lifecycle-owner.ini".to_string(),
        source,
    )])
    .unwrap();
    let mut world = GameLogic::new();
    world.frame = frame;
    world.admit_receiving_module_content(content).unwrap();
    let id = world.create_object(name, Team::USA, position).unwrap();
    (world, id)
}

fn drive(world: &mut GameLogic, id: ObjectId, frame: u32) {
    world.frame = frame;
    world.tick_native_lifecycle(id).unwrap();
}

#[test]
fn neutral_authored_timers_start_at_construction_and_round_duration_up() {
    for (module, deletion) in [("LifetimeUpdate", false), ("DeletionUpdate", true)] {
        let (mut world, id) = authored(
            "NeutralTimedOwner",
            &format!("Behavior = {module} ClockTag\nMinLifetime = 34\nMaxLifetime = 34\nEnd"),
            30,
            Vec3::ZERO,
        );
        let runtime = world
            .host_object(id)
            .unwrap()
            .native_lifecycle
            .as_ref()
            .unwrap();
        assert_eq!(runtime.states.len(), 1);
        match &runtime.states[0] {
            NativeLifecycleState::Lifetime { die_frame, active } => {
                assert!(!deletion);
                assert_eq!(*die_frame, 32);
                assert!(*active);
            }
            NativeLifecycleState::Deletion { die_frame, active } => {
                assert!(deletion);
                assert_eq!(*die_frame, 32);
                assert!(*active);
            }
            _ => panic!("receiving timer has the authored update kind"),
        }
        drive(&mut world, id, 31);
        assert!(!world.host_object(id).unwrap().status.destroyed);
        drive(&mut world, id, 32);
        let object = world.host_object(id).unwrap();
        // LifetimeUpdate.cpp69-73 kills, not deletes; Object::onDie only
        // visits actual Die modules. This authored fixture has none.
        if deletion {
            assert!(object.status.destroyed);
        } else {
            assert!(object.status.effectively_dead);
        }
        assert_eq!(object.status.on_die_started, !deletion);
        assert_eq!(object.health.current, if deletion { 100.0 } else { 0.0 });
        assert_eq!(
            world
                .objects_to_destroy
                .iter()
                .filter(|event| event.id == id)
                .count(),
            usize::from(deletion)
        );
    }
}

#[test]
fn rejected_lifetime_is_consumed_while_deletion_ignores_body_indestructibility() {
    for (module, deletion) in [("LifetimeUpdate", false), ("DeletionUpdate", true)] {
        let (mut world, id) = authored(
            "NeutralInvulnerableTimer",
            &format!("Behavior = {module} TimerTag\nMinLifetime = 0\nMaxLifetime = 0\nEnd"),
            30,
            Vec3::ZERO,
        );
        world.host_object_mut(id).unwrap().indestructible = true;
        drive(&mut world, id, 31);
        let object = world.host_object(id).unwrap();
        assert_eq!(object.status.destroyed, deletion);
        assert!(!object.status.on_die_started);
        assert_eq!(object.health.current, 100.0);
        match &object.native_lifecycle.as_ref().unwrap().states[0] {
            NativeLifecycleState::Lifetime { active, .. }
            | NativeLifecycleState::Deletion { active, .. } => assert!(!active),
            _ => panic!("timer kind retained"),
        }
        if !deletion {
            world.host_object_mut(id).unwrap().indestructible = false;
            drive(&mut world, id, 99);
            assert!(!world.host_object(id).unwrap().status.on_die_started);
            assert_eq!(world.host_object(id).unwrap().health.current, 100.0);
        }
    }
}

#[test]
fn two_authored_timer_modules_keep_independent_deadlines_and_source_order() {
    let (mut world, id) = authored(
        "NeutralTwoTimers",
        "Behavior = LifetimeUpdate FirstTag\nMinLifetime = 34\nMaxLifetime = 34\nEnd\nBehavior = DeletionUpdate SecondTag\nMinLifetime = 100\nMaxLifetime = 100\nEnd",
        30,
        Vec3::ZERO,
    );
    let states = &world
        .host_object(id)
        .unwrap()
        .native_lifecycle
        .as_ref()
        .unwrap()
        .states;
    assert_eq!(states.len(), 2);
    assert!(matches!(
        states[0],
        NativeLifecycleState::Lifetime {
            die_frame: 32,
            active: true
        }
    ));
    assert!(matches!(
        states[1],
        NativeLifecycleState::Deletion {
            die_frame: 33,
            active: true
        }
    ));
    world.host_object_mut(id).unwrap().indestructible = true;
    drive(&mut world, id, 32);
    assert!(!world.host_object(id).unwrap().status.destroyed);
    drive(&mut world, id, 33);
    let object = world.host_object(id).unwrap();
    assert!(object.status.destroyed);
    assert!(!object.status.on_die_started);
    assert_eq!(object.health.current, 100.0);
}

#[test]
fn bound_empty_native_definition_does_not_install_name_peel_timers() {
    let (mut world, id) = authored("PoisonFieldMedium", "", 30, Vec3::ZERO);
    let object = world.host_object(id).unwrap();
    assert!(object.native_lifecycle.as_ref().unwrap().states.is_empty());
    drive(&mut world, id, 100_000);
    world
        .update_ai(&[id], 1.0 / 30.0)
        .expect("receiving AI operation succeeds");
    let object = world.host_object(id).unwrap();
    assert!(object.lifetime_update.is_none());
    assert!(object.height_die.is_none());
    assert!(!object.status.on_die_started && !object.status.destroyed);
}

#[test]
fn admitted_empty_lifecycle_ai_needs_no_legacy_terrain_sample() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "admitted_empty_lifecycle_ai_needs_no_legacy_terrain_sample",
        || {
            let (mut world, id) = authored("NapalmBomb", "", 30, Vec3::ZERO);
            let terrain = std::sync::Arc::clone(world.world_services.terrain());
            let terrain_write = terrain.write().unwrap();
            world.update_ai(&[id], 1.0 / 30.0).unwrap();
            drop(terrain_write);

            let object = world.host_object(id).unwrap();
            assert!(object.native_lifecycle.as_ref().unwrap().states.is_empty());
            assert!(object.height_die.is_none());
            assert_eq!(object.health.current, 100.0);
            assert!(!object.status.on_die_started);
        },
    );
}

#[test]
fn authored_crate_births_use_one_object_clock_or_none() {
    use crate::game_logic::host_deliver_payload::HostDeliverPayloadKind;
    use crate::save_load::snapshot::SnapshotBuilder;
    use game_engine::common::ini::{ParsedCrateCreationEntry, ParsedCrateTemplate};

    const FIXED: &str = "ClockFixedCrate";
    const RANGED: &str = "ClockRangedCrate";
    const CLOCKLESS: &str = "SupplyDropZoneCrate";
    const BIRTH: u32 = 120;

    let fixture = || {
        let mut world = GameLogic::new();
        let mut ini = String::new();
        for (name, range) in [
            (FIXED, Some((1_000, 1_000))),
            (RANGED, Some((60_000, 62_000))),
            (CLOCKLESS, None),
        ] {
            ini.push_str(&format!(
                "Object {name}\nKindOf = CRATE PARACHUTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\n"
            ));
            if let Some((min, max)) = range {
                ini.push_str(&format!(
                    "Behavior = DeletionUpdate TimerTag\nMinLifetime = {min}\nMaxLifetime = {max}\nEnd\n"
                ));
            }
            ini.push_str("End\n");
        }
        world
            .admit_object_ini_sources([("authored-crate-clocks.ini".into(), ini)])
            .unwrap();
        world.add_player(Player::new(1, Team::USA, "CargoOwner", false));
        world.frame = BIRTH;
        world
    };

    let mut world = fixture();
    let mut expected = world.logic_random.snapshot();
    let mut clocks = Vec::new();
    for (name, lo, hi) in [(FIXED, 30, 30), (RANGED, 1800, 1860)] {
        let key = format!("{name}Data");
        let mut data = ParsedCrateTemplate::new(key.clone());
        data.creation_chance = 1.0;
        data.possible_crates.push(ParsedCrateCreationEntry {
            crate_name: name.into(),
            crate_chance: 1.0,
        });
        world
            .world_services
            .common()
            .ini
            .crate_system
            .write()
            .insert(data);
        assert_eq!(
            world.try_create_crates_on_die(ObjectId(1234), Vec3::ZERO, Team::USA, &[key], None,),
            1,
        );
        let object = world
            .host_objects()
            .registered_values()
            .find(|object| object.template_name == name)
            .unwrap();
        let frame = BIRTH + expected.next_int(lo, hi) as u32;
        assert_eq!(
            object.native_lifecycle.as_ref().unwrap().states[0],
            NativeLifecycleState::Deletion {
                die_frame: frame,
                active: true
            },
        );
        clocks.push((object.id, frame));
    }

    let kind = HostDeliverPayloadKind::SupplyDropZoneCrate;
    let mission = world.host_deliver_payloads.queue_for_owner(
        kind,
        ObjectId(1235),
        Team::USA,
        Some(1),
        Vec3::new(70., 0., 80.),
        0,
        CLOCKLESS,
    );
    world.update_deliver_payloads();
    let cargo = world
        .host_deliver_payloads
        .get(mission)
        .unwrap()
        .spawned_payload_ids[0];
    let object = world.host_object(cargo).unwrap();
    assert_eq!(object.template_name, CLOCKLESS);
    assert!(object.native_lifecycle.as_ref().unwrap().states.is_empty());
    assert_eq!(world.logic_random.seed_words(), expected.seed_words());

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&world).unwrap();
    let mut loaded = fixture();
    builder
        .restore_from_snapshot(&snapshot, &mut loaded)
        .unwrap();
    assert_eq!(loaded.logic_random.seed_words(), expected.seed_words());
    let [(fixed, fixed_due), (ranged, ranged_due)] = clocks.try_into().unwrap();
    for (frame, fixed_live, ranged_live) in [
        (fixed_due - 1, true, true),
        (fixed_due, false, true),
        (BIRTH + 1350, false, true),
        (ranged_due - 1, false, true),
        (ranged_due, false, false),
    ] {
        loaded.set_current_frame(u64::from(frame));
        loaded.tick_logic_frame(1. / 30., None, Some(1));
        assert_eq!(loaded.frame, frame + 1);
        assert_eq!(loaded.host_object(fixed).is_some(), fixed_live);
        assert_eq!(loaded.host_object(ranged).is_some(), ranged_live);
        assert!(loaded.host_object(cargo).is_some());
    }
    assert!(!loaded.host_money_crates.contains(fixed));
    assert!(!loaded.host_money_crates.contains(ranged));
    assert!(loaded.host_money_crates.contains(cargo));
}

#[test]
fn mounted_crate_ini_objects_keep_their_authored_birth_and_lifetime() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "mounted_crate_ini_objects_keep_their_authored_birth_and_lifetime",
        || {
            let startup = crate::parallel_harness::RetailWorldDefinitions::load().unwrap();
            let mut world = GameLogic::new();
            startup.admit(&mut world).unwrap();
            world.frame = 40;
            let mut expected = world.logic_random.snapshot();

            // INIZH.big Data/INI/Crate.ini:182,209–211. Only SalvageCrate
            // carries DeletionUpdate. Its body can be destroyed by the module.
            let salvage = world
                .create_object_with_script_effects_checked(
                    "SalvageCrate", Team::Neutral, Vec3::ZERO, None, &mut Default::default(),
                )
                .unwrap()
                .expect("stock SalvageCrate is a receiving ThingFactory birth");
            let due = 40 + expected.next_int(900, 1050) as u32;
            let object = world.host_object(salvage).unwrap();
            assert!(object.is_kind_of(KindOf::Selectable));
            assert!(!object.is_kind_of(KindOf::Crate));
            assert_eq!(
                object.native_lifecycle.as_ref().unwrap().states.as_ref(),
                &[NativeLifecycleState::Deletion { die_frame: due, active: true }],
            );

            // Stock 200DollarCrate and SupplyDropZoneCrate have an explicitly
            // empty native lifecycle and consume no lifetime RNG draws.
            let cash = ["200DollarCrate", "SupplyDropZoneCrate"].map(|name| {
                let id = world
                    .create_object_with_script_effects_checked(
                        name, Team::Neutral, Vec3::ZERO, None, &mut Default::default(),
                    )
                    .unwrap()
                    .expect("stock money crate is a receiving ThingFactory birth");
                let object = world.host_object(id).unwrap();
                assert!(object.is_kind_of(KindOf::Crate));
                assert!(object.native_lifecycle.as_ref().unwrap().states.is_empty());
                id
            });
            assert_eq!(world.logic_random.seed_words(), expected.seed_words());
            let ids = [salvage, cash[0], cash[1]];
            world.frame = due - 1;
            world.update_ai(&ids, 1. / 30.).unwrap();
            assert!(!world.host_object(salvage).unwrap().status.destroyed);
            world.frame = due;
            world.update_ai(&ids, 1. / 30.).unwrap();
            let object = world.host_object(salvage).unwrap();
            assert!(object.status.destroyed && !object.status.on_die_started);
            world.process_destroy_list();
            assert!(world.host_object(salvage).is_none());
            for id in cash {
                assert!(world.host_object(id).is_some_and(|object| !object.status.destroyed));
            }
        },
    );
}

#[test]
fn authored_crate_die_rolls_each_resolved_data_row_in_die_order() {
    use crate::game_logic::{
        combat::DamageType, host_usa_pilot::HostDeathType, object::DamageHitContext,
    };
    use game_engine::common::system::game_common::normalize_angle;

    let position = Vec3::new(35., 12., 47.);
    let (mut world, victim) = authored_with_extra(
        "CrateClockVictim",
        "Behavior = CreateCrateDie DropTag\n\
         CrateData = Missing\nCrateData = ZeroChance\nCrateData = NoWeight\n\
         CrateData = SpawnClock\nCrateData = SpawnClock\nEnd",
        40,
        position,
        "Object ClockCrate\nKindOf = IMMOBILE\n\
         Body = InactiveBody BodyTag\nEnd\n\
         Behavior = DeletionUpdate ClockTag\nMinLifetime = 1000\nMaxLifetime = 2000\nEnd\nEnd",
    );
    let sources = "CrateData DefaultCrate\nCreationChance = 0\nEnd\n\
         CrateData ZeroChance\nCrateObject = ClockCrate 1\nEnd\n\
         CrateData NoWeight\nCreationChance = 1\nCrateObject = ClockCrate 0\nEnd\n\
         CrateData SpawnClock\nCreationChance = 1\nCrateObject = ClockCrate 1\nEnd\n";
    world
        .world_services
        .common()
        .ini
        .crate_system
        .write()
        .apply_mixed_source(sources)
        .unwrap();
    world.set_owned_pathfinder_layer(victim, 2);
    world.set_logic_random_seed(0xC8A7E);
    let mut expected = world.logic_random.snapshot();
    expected.next_real(0., 1.); // Inherited zero chance, no pick.
    assert!(expected.next_real(0., 1.) < 1.);
    expected.next_real(0., 1.); // Existing row with no weighted selection.
    let expected_outcomes = std::array::from_fn::<_, 2, _>(|_| {
        assert!(expected.next_real(0., 1.) < 1.);
        assert!(expected.next_real(0., 1.) < 1.);
        let due = 40 + expected.next_int(30, 60) as u32;
        let orientation = normalize_angle(expected.next_real(0., std::f32::consts::TAU));
        (due, orientation)
    });

    world
        .apply_owned_damage_with_effects(
            victim,
            200.,
            None,
            DamageType::Unresistable,
            HostDeathType::Normal,
            None,
            &DamageHitContext::default(),
            &mut Default::default(),
        )
        .unwrap()
        .expect("lethal receiving Body application");

    let drops = world
        .objects
        .registered_values()
        .rev()
        .filter(|o| o.template_name == "ClockCrate")
        .collect::<Vec<_>>();
    assert_eq!(drops.len(), expected_outcomes.len());
    for (object, (due, angle)) in drops.into_iter().zip(expected_outcomes) {
        assert_eq!(object.get_position(), position);
        assert_eq!(object.get_orientation(), angle);
        assert_eq!(object.pathfind_layer, 2);
        assert_eq!(
            object.native_lifecycle.as_ref().unwrap().states.as_ref(),
            &[NativeLifecycleState::Deletion {
                die_frame: due,
                active: true
            }],
        );
    }
    assert_eq!(world.logic_random.seed_words(), expected.seed_words());
}

#[test]
fn authored_ai_consumes_crate_notification_without_chasing() {
    let (mut world, picker) = authored_with_extra(
        "PickupMarkerOwner",
        "Behavior = AIUpdateInterface PickupAI\nEnd",
        40,
        Vec3::ZERO,
        "Object PickupMarkerActiveCrate\nKindOf = SELECTABLE CRATE\n\
         Body = ActiveBody PickupBody\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd",
    );
    let crate_id = world
        .create_object("PickupMarkerActiveCrate", Team::Neutral, Vec3::X * 30.)
        .unwrap();
    assert!(world.host_object(picker).unwrap().has_ai_update_interface());
    assert!(world.host_object(crate_id).unwrap().is_alive());
    assert!(world.notify_unit_crate(picker, crate_id));
    assert_eq!(
        world.host_object(picker).unwrap().crate_created,
        Some(crate_id)
    );

    assert!(!world.try_idle_crate_pickup(picker));
    let owner = world.host_object(picker).unwrap();
    assert_eq!(owner.crate_created, None);
    assert_eq!(owner.requested_destination, None);
}

#[test]
fn mounted_stock_scorpion_collects_authored_salvage_at_partition_contact() {
    use crate::game_logic::{
        ScriptPresentationEffects, VeterancyLevel,
        presentation_definitions::PresentationDefinitions,
    };
    use game_engine::common::system::{
        archive_file_system::ArchiveFileSystem, install_layout::find_file_case_insensitive,
    };
    use generals_sim::weapon::WeaponSetType;
    use std::io::Read;

    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "mounted_stock_scorpion_collects_authored_salvage_at_partition_contact",
        || {
            let install = std::env::var_os("GENERALS_RETAIL_CONTENT_ROOT")
                .map(std::path::PathBuf::from)
                .or_else(crate::map_frame_scenario::find_lone_eagle_retail_install)
                .expect("a real mounted retail install is required");
            let install = crate::parallel_harness::explicit_retail_install(&install).unwrap();
            let startup =
                crate::parallel_harness::RetailWorldDefinitions::load_from_install(Some(&install))
                    .unwrap_or_else(|error| panic!("{}: {error}", install.display()));
            let mut world = GameLogic::new();
            startup.admit(&mut world).unwrap();
            let ini_archive = [&install, &install.join("Data")]
                .into_iter()
                .find_map(|dir| find_file_case_insensitive(dir, "INIZH.big"))
                .unwrap();
            let mut archive = ArchiveFileSystem::new();
            archive
                .open_archive_file(&ini_archive.to_string_lossy())
                .unwrap();
            let misc_path = "Data/INI/MiscAudio.ini";
            let mut misc = Vec::new();
            archive
                .open_file(misc_path, 1)
                .unwrap()
                .read_to_end(&mut misc)
                .unwrap();
            world
                .admit_presentation_definitions(
                    PresentationDefinitions::from_ordered_sources(
                        &[],
                        &[(misc_path, String::from_utf8_lossy(&misc).into_owned())],
                        &[],
                        &|_| false,
                    )
                    .unwrap(),
                )
                .unwrap();
            world.add_player(Player::new(1, Team::GLA, "NativeSalvager", true));
            world.frame = 40;

            let mut effects = ScriptPresentationEffects::default();
            let at_crate = Vec3::new(200., 0., 200.);
            let away = at_crate - Vec3::X * 45.;
            let picker = world
                .create_object_for_player_with_effects_checked(
                    "GLATankScorpion",
                    1,
                    away,
                    None,
                    &mut effects,
                )
                .unwrap()
                .expect("stock GLA Scorpion");
            let position =
                |world: &mut GameLogic, effects: &mut ScriptPresentationEffects<'_>, point| {
                    assert!(world.set_owned_object_position_with_effects(
                        picker,
                        point,
                        world.frame,
                        effects
                    ));
                    world.update_owned_partition_with_effects(effects).unwrap();
                };
            let birth = |world: &mut GameLogic, effects: &mut ScriptPresentationEffects<'_>| {
                let id = world
                    .create_object_with_script_effects_checked(
                        "SalvageCrate",
                        Team::Neutral,
                        at_crate,
                        None,
                        effects,
                    )
                    .unwrap()
                    .expect("stock authored crate");
                let crate_object = world.host_object(id).unwrap();
                assert!(!crate_object.status.effectively_dead);
                assert_eq!(world.presentation_crate_flags(crate_object), (true, true));
                id
            };
            let assert_retired = |world: &mut GameLogic, id| {
                assert!(world.host_object(id).unwrap().status.destroyed);
                world.process_destroy_list();
                assert!(world.host_object(id).is_none());
            };

            let first = birth(&mut world, &mut effects);
            let untouched = world.logic_random.seed_words();
            position(&mut world, &mut effects, at_crate);
            assert!(!world.host_object(first).unwrap().status.destroyed);
            assert_eq!(world.logic_random.seed_words(), untouched);
            assert_eq!(world.host_object(picker).unwrap().weapon_crate_upgrade(), 0);
            assert!(
                world
                    .get_player_mut(1)
                    .unwrap()
                    .unlock_science("SCIENCE_GLA")
            );
            position(&mut world, &mut effects, away);
            assert!(!world.host_object(first).unwrap().status.destroyed);

            let funds = world.get_player(1).unwrap().resources.supplies;
            position(&mut world, &mut effects, at_crate);
            assert_eq!(world.logic_random.seed_words(), untouched);
            assert_eq!(world.get_player(1).unwrap().resources.supplies, funds);
            let scorpion = world.host_object(picker).unwrap();
            assert!(scorpion.test_weapon_condition(WeaponSetType::CrateUpgradeOne as u8));
            assert!(!scorpion.test_weapon_condition(WeaponSetType::CrateUpgradeTwo as u8));
            assert_eq!(
                scorpion.selected_weapon_name(0),
                Some("ScorpionTankGunPlusOne")
            );
            assert_eq!(scorpion.selected_weapon_name(1), None);
            assert!(world.queued_audio_events.iter().any(|event| {
                event.event_type == "SalvageUpgrade" && event.object_id == Some(picker)
            }));
            assert_retired(&mut world, first);

            position(&mut world, &mut effects, away);
            let second = birth(&mut world, &mut effects);
            let untouched = world.logic_random.seed_words();
            position(&mut world, &mut effects, at_crate);
            assert_eq!(world.logic_random.seed_words(), untouched);
            let scorpion = world.host_object(picker).unwrap();
            assert!(!scorpion.test_weapon_condition(WeaponSetType::CrateUpgradeOne as u8));
            assert!(scorpion.test_weapon_condition(WeaponSetType::CrateUpgradeTwo as u8));
            assert_eq!(
                scorpion.selected_weapon_name(0),
                Some("ScorpionTankGunPlusOne")
            );
            assert_retired(&mut world, second);

            for (seed, gain_level) in [(0x2000_0000, true), (0, false)] {
                position(&mut world, &mut effects, away);
                let id = birth(&mut world, &mut effects);
                world.set_logic_random_seed(seed);
                let mut expected = world.logic_random.snapshot();
                assert_eq!(expected.next_real(0., 1.) < 0.25, gain_level);
                let expected_money = if gain_level {
                    0
                } else {
                    expected.next_int(25, 75) as u32
                };
                let earned = world.get_player(1).unwrap().statistics.money_earned();
                world.queued_audio_events.clear();
                position(&mut world, &mut effects, at_crate);
                assert_eq!(
                    world.host_object(picker).unwrap().experience.level,
                    VeterancyLevel::Veteran
                );
                assert_eq!(world.logic_random.seed_words(), expected.seed_words());
                assert_eq!(
                    world.get_player(1).unwrap().resources.supplies,
                    funds + expected_money
                );
                assert_eq!(
                    world.get_player(1).unwrap().statistics.money_earned(),
                    earned + expected_money
                );
                if !gain_level {
                    assert!(world.queued_audio_events.iter().any(|event| {
                        event.event_type == "CrateMoney" && event.object_id == Some(picker)
                    }));
                    let floating = world.host_money_crates.money_floating_texts.last().unwrap();
                    assert_eq!(floating.amount, expected_money);
                    assert_eq!(floating.position, at_crate + Vec3::Y * 10.);
                    assert_eq!(floating.picker_id, picker);
                }
                assert_retired(&mut world, id);
            }
        },
    );
}

#[test]
fn authored_height_die_uses_its_own_clock_and_sample_through_ai() {
    let (mut world, id) = authored(
        "NativeAltitudeOwner",
        "Behavior = HeightDieUpdate HeightTag\nTargetHeight = 10\nInitialDelay = 34\n\
         OnlyWhenMovingDown = Yes\nSnapToGroundOnDeath = Yes\nEnd",
        30,
        Vec3::new(0.0, 13.0, 0.0),
    );
    let (peer, peer_id) = authored("NativeAltitudeOwner", "", 30, Vec3::ZERO);
    assert_eq!(id, peer_id);
    world.host_object_mut(id).unwrap().ground_height = 4.0;

    for frame in [40, 41, 42] {
        world.frame = frame;
        world.update_ai(&[id], 1.0 / 30.0).unwrap();
        let object = world.host_object(id).unwrap();
        assert_eq!(object.health.current, 100.0);
        assert!(object.height_die.is_none());
        let expected_position = if frame < 42 {
            Vec3::splat(-1.0)
        } else {
            Vec3::new(0.0, 13.0, 0.0)
        };
        assert!(matches!(
            object.native_lifecycle.as_ref().unwrap().states[0],
            NativeLifecycleState::HeightDie {
                earliest_frame: 42,
                last_position,
                has_died: false,
                ..
            } if last_position == expected_position
        ));
    }

    world.set_fixture_object_position(id, Vec3::new(0.0, 5.0, 0.0));
    world.frame = 43;
    world.update_ai(&[id], 1.0 / 30.0).unwrap();
    let object = world.host_object(id).unwrap();
    assert_eq!(object.health.current, 0.0);
    assert_eq!(object.get_position(), Vec3::new(0.0, 4.0, 0.0));
    assert!(object.status.on_die_started);
    assert_eq!(peer.host_object(peer_id).unwrap().health.current, 100.0);
}

#[test]
fn restored_legacy_height_die_still_samples_terrain_through_ai() {
    use crate::game_logic::host_height_die::HostHeightDieData;
    use crate::save_load::snapshot::{SnapshotBuilder, decode_bincode_world_snapshot};

    let mut source = GameLogic::new();
    let name = "LegacyAltitudeOwner";
    let mut template = ThingTemplate::new(name);
    template.set_health(100.0);
    source.templates.insert(name.into(), template.clone());
    let id = source
        .create_object(name, Team::USA, Vec3::new(0.0, 22.0, 0.0))
        .unwrap();
    let object = source.host_object_mut(id).unwrap();
    assert!(object.native_lifecycle.is_none());
    object.ground_height = 5.0;
    object.height_die = Some(HostHeightDieData::with_target(2.0, true, 0));
    source.frame = 40;
    source.update_ai(&[id], 1.0 / 30.0).unwrap();
    assert_eq!(
        source
            .host_object(id)
            .unwrap()
            .height_die
            .as_ref()
            .unwrap()
            .last_height,
        17.0
    );

    let builder = SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let snapshot = decode_bincode_world_snapshot(&bytes).unwrap();
    let mut receiving = GameLogic::new();
    receiving.templates.insert(name.into(), template);
    builder
        .restore_from_snapshot(&snapshot, &mut receiving)
        .unwrap();
    let restored = receiving.host_object_mut(id).unwrap();
    assert!(restored.native_lifecycle.is_none());
    assert_eq!(restored.height_die.as_ref().unwrap().last_height, 17.0);
    restored.ground_height = 5.0;

    receiving.set_fixture_object_position(id, Vec3::new(0.0, 6.0, 0.0));
    receiving.frame = 41;
    receiving.update_ai(&[id], 1.0 / 30.0).unwrap();
    let restored = receiving.host_object(id).unwrap();
    assert_eq!(restored.health.current, 0.0);
    assert!(restored.height_die.as_ref().unwrap().has_died);
    assert_eq!(source.host_object(id).unwrap().health.current, 100.0);
}

#[test]
fn height_threshold_is_strict_and_descent_observes_absolute_position() {
    let (mut world, id) = authored(
        "NeutralHeightOwner",
        "Behavior = HeightDieUpdate HeightTag\nTargetHeight = 10\nOnlyWhenMovingDown = Yes\nEnd",
        30,
        Vec3::new(0.0, 10.0, 0.0),
    );
    drive(&mut world, id, 30);
    assert!(
        !world.host_object(id).unwrap().status.on_die_started,
        "initial sentinel is minus one, not positive infinity"
    );
    world.set_fixture_object_position(id, Vec3::new(0.0, 11.0, 0.0));
    drive(&mut world, id, 31);
    world.set_fixture_object_position(id, Vec3::new(0.0, 10.0, 0.0));
    drive(&mut world, id, 32);
    assert!(
        !world.host_object(id).unwrap().status.on_die_started,
        "equal target does not die"
    );
    world.host_object_mut(id).unwrap().ground_height = 9.0;
    drive(&mut world, id, 33);
    assert!(
        !world.host_object(id).unwrap().status.on_die_started,
        "rising terrain is not downward movement"
    );
    world.set_fixture_object_position(id, Vec3::new(0.0, 9.0, 0.0));
    drive(&mut world, id, 34);
    assert!(world.host_object(id).unwrap().status.on_die_started);
}

#[test]
fn height_delay_begins_at_first_update_and_rejected_kill_does_not_rearm() {
    let (mut world, id) = authored(
        "NeutralDelayedHeightOwner",
        "Behavior = HeightDieUpdate HeightTag\nTargetHeight = 10\nInitialDelay = 34\nSnapToGroundOnDeath = Yes\nEnd",
        30,
        Vec3::new(0.0, 5.0, 0.0),
    );
    world.host_object_mut(id).unwrap().indestructible = true;
    drive(&mut world, id, 40);
    match &world
        .host_object(id)
        .unwrap()
        .native_lifecycle
        .as_ref()
        .unwrap()
        .states[0]
    {
        NativeLifecycleState::HeightDie {
            earliest_frame,
            last_position,
            has_died,
            ..
        } => {
            assert_eq!(*earliest_frame, 42);
            assert_eq!(*last_position, Vec3::splat(-1.0));
            assert!(!has_died);
        }
        _ => panic!("authored height identity retained"),
    }
    drive(&mut world, id, 41);
    assert_eq!(world.host_object(id).unwrap().get_position().y, 5.0);
    drive(&mut world, id, 42);
    let object = world.host_object(id).unwrap();
    assert_eq!(
        object.get_position().y,
        0.0,
        "snap happens even when Body rejects kill"
    );
    assert_eq!(object.health.current, 100.0);
    assert!(!object.status.on_die_started);
    assert!(matches!(
        object.native_lifecycle.as_ref().unwrap().states[0],
        NativeLifecycleState::HeightDie { has_died: true, .. }
    ));
    world.host_object_mut(id).unwrap().indestructible = false;
    drive(&mut world, id, 43);
    assert!(!world.host_object(id).unwrap().status.on_die_started);
}

#[test]
fn height_defaults_allow_stationary_death_and_keep_negative_target() {
    for (fields, initial, later) in [("", -0.5, -0.5), ("TargetHeight = -2", -1.0, -3.0)] {
        let (mut world, id) = authored(
            "NeutralDefaultHeightOwner",
            &format!("Behavior = HeightDieUpdate HeightTag\n{fields}\nEnd"),
            30,
            Vec3::new(0.0, initial, 0.0),
        );
        drive(&mut world, id, 30);
        if fields.is_empty() {
            let object = world.host_object(id).unwrap();
            assert!(object.status.on_die_started);
            assert_eq!(
                object.get_position().y,
                0.0,
                "below-ground clamping precedes death even without SnapToGround"
            );
        } else {
            assert!(
                !world.host_object(id).unwrap().status.on_die_started,
                "negative authored TargetHeight must not be clamped to zero"
            );
            world.set_fixture_object_position(id, Vec3::new(0.0, later, 0.0));
            drive(&mut world, id, 31);
            assert!(world.host_object(id).unwrap().status.on_die_started);
        }
    }
}

#[test]
fn delayed_containment_does_not_update_last_position_before_admission() {
    let (mut world, id) = authored(
        "NeutralContainedHeightOwner",
        "Behavior = HeightDieUpdate HeightTag\nTargetHeight = 10\nOnlyWhenMovingDown = Yes\nInitialDelay = 34\nEnd",
        30,
        Vec3::new(0.0, 5.0, 0.0),
    );
    world.set_fixture_object_contained_by(id, Some(ObjectId(999)));
    drive(&mut world, id, 40);
    match &world
        .host_object(id)
        .unwrap()
        .native_lifecycle
        .as_ref()
        .unwrap()
        .states[0]
    {
        NativeLifecycleState::HeightDie { last_position, .. } => {
            assert_eq!(*last_position, Vec3::splat(-1.0))
        }
        _ => panic!("height module retained"),
    }
    drive(&mut world, id, 42);
    match &world
        .host_object(id)
        .unwrap()
        .native_lifecycle
        .as_ref()
        .unwrap()
        .states[0]
    {
        NativeLifecycleState::HeightDie {
            last_position,
            has_died,
            ..
        } => {
            assert_eq!(*last_position, Vec3::new(0.0, 5.0, 0.0));
            assert!(!has_died);
        }
        _ => panic!("height module retained"),
    }
    world.set_fixture_object_contained_by(id, None);
    drive(&mut world, id, 43);
    assert!(
        !world.host_object(id).unwrap().status.on_die_started,
        "unloading without descent does not die"
    );
    world.set_fixture_object_position(id, Vec3::new(0.0, 4.0, 0.0));
    drive(&mut world, id, 44);
    assert!(world.host_object(id).unwrap().status.on_die_started);
}

#[test]
fn height_particle_cleanup_observes_snap_after_rejected_body_and_only_attachments() {
    use crate::game_logic::combat_particles::CombatParticleKind;
    let (mut world, id) = authored(
        "NeutralParticleHeightOwner",
        "Behavior = HeightDieUpdate HeightTag\nTargetHeight = 10\nSnapToGroundOnDeath = Yes\nDestroyAttachedParticlesAtHeight = 1\nEnd",
        30,
        Vec3::new(0.0, 5.0, 0.0),
    );
    world.host_object_mut(id).unwrap().indestructible = true;
    let peer = ObjectId(999);
    let mut attached = Vec::new();
    for kind in [
        CombatParticleKind::ParticleSysBone,
        CombatParticleKind::BodyFire,
        CombatParticleKind::BodySmoke,
        CombatParticleKind::DisableFx,
    ] {
        attached.push(
            world
                .combat_particles
                .attach_named_to_object_local(
                    id,
                    Vec3::new(0.0, 5.0, 0.0),
                    0.0,
                    Vec3::ZERO,
                    30,
                    "HeightCleanupAttachedWitness",
                    kind,
                    None,
                    &mut (),
                )
                .unwrap(),
        );
    }
    attached.push(
        world
            .combat_particles
            .spawn_projectile_exhaust(
                Vec3::new(0.0, 5.0, 0.0),
                30,
                peer,
                Some(id),
                "HeightCleanupExhaustWitness",
                &mut (),
            )
            .unwrap(),
    );
    let peer_attached = world
        .combat_particles
        .attach_named_to_object(peer, Vec3::ZERO, 30, "HeightCleanupPeerWitness", &mut ())
        .unwrap();
    let impact = world.combat_particles.spawn_named(
        CombatParticleKind::WeaponImpact,
        "HeightCleanupTargetImpactWitness",
        Vec3::new(0.0, 5.0, 0.0),
        30,
        Some(peer),
        Some(id),
        &mut (),
    );
    drive(&mut world, id, 30);
    let object = world.host_object(id).unwrap();
    assert_eq!(object.get_position(), Vec3::ZERO);
    assert_eq!(object.health.current, 100.0);
    assert!(!object.status.on_die_started);
    match &object.native_lifecycle.as_ref().unwrap().states[0] {
        NativeLifecycleState::HeightDie {
            has_died,
            particles_destroyed,
            last_position,
            ..
        } => {
            assert!(*has_died && *particles_destroyed);
            assert_eq!(
                *last_position,
                Vec3::ZERO,
                "continuation captures the snapped live pose"
            );
        }
        _ => panic!("height module retained"),
    }
    let active: Vec<_> = world
        .combat_particles
        .active_systems()
        .map(|entry| entry.id)
        .collect();
    for attachment in attached {
        assert!(!active.contains(&attachment));
    }
    assert!(
        active.contains(&peer_attached),
        "another receiving owner retains its particles"
    );
    assert!(
        active.contains(&impact),
        "damage target association does not mean attachment"
    );
    let later = world
        .combat_particles
        .attach_named_to_object(id, Vec3::ZERO, 31, "HeightCleanupLaterWitness", &mut ())
        .unwrap();
    drive(&mut world, id, 31);
    assert!(
        world
            .combat_particles
            .active_systems()
            .any(|entry| entry.id == later),
        "once-only cleanup latch does not destroy later attachments"
    );
}

#[test]
fn hulk_override_is_kind_based_constructor_only_and_first_lifetime_range_is_local() {
    let (mut owner, _ordinary) = authored_with_extra(
        "HulkSpellingIsNotAKind",
        "Behavior = LifetimeUpdate First\nMinLifetime = 34\nMaxLifetime = 34\nEnd\nBehavior = LifetimeUpdate Second\nMinLifetime = 95\nMaxLifetime = 95\nEnd\nBehavior = DeletionUpdate Delete\nMinLifetime = 200\nMaxLifetime = 200\nEnd",
        30,
        Vec3::ZERO,
        "ChildObject NeutralDebris HulkSpellingIsNotAKind\nKindOf = IMMOBILE INERT NO_COLLIDE HULK\nEnd",
    );
    owner.native_hulk_max_lifetime_override = 9;
    let named = owner
        .create_object("HulkSpellingIsNotAKind", Team::Neutral, Vec3::ZERO)
        .unwrap();
    let deadline = |world: &GameLogic, id, index| match world
        .host_object(id)
        .unwrap()
        .native_lifecycle
        .as_ref()
        .unwrap()
        .states[index]
    {
        NativeLifecycleState::Lifetime { die_frame, .. }
        | NativeLifecycleState::Deletion { die_frame, .. } => die_frame,
        _ => panic!("expected timer"),
    };
    assert_eq!(deadline(&owner, named, 0), 32);
    let id = owner
        .create_object("NeutralDebris", Team::Neutral, Vec3::ZERO)
        .unwrap();
    assert_eq!(deadline(&owner, id, 0), 39);
    assert_eq!(deadline(&owner, id, 1), 39);
    assert_eq!(deadline(&owner, id, 2), 36); // Deletion never applies the HULK override.
    let random = owner.logic_random.clone();
    owner
        .host_object_mut(id)
        .unwrap()
        .set_native_lifetime_range(
            30,
            crate::game_logic::native_lifecycle_definition::LifetimeFrameRange {
                minimum: 4,
                maximum: 4,
            },
            &random,
        );
    assert_eq!(deadline(&owner, id, 0), 34);
    assert_eq!(deadline(&owner, id, 1), 39);
    let mut peer = GameLogic::new();
    peer.native_hulk_max_lifetime_override = 77;
    peer.reset();
    assert_eq!(peer.native_hulk_max_lifetime_override, -1);
    assert_eq!(owner.native_hulk_max_lifetime_override, 9);
}

#[test]
fn height_structure_query_uses_receiving_geometry_centers_and_strict_3d_range() {
    fn scene(height: f32, x: f32, base_y: f32) -> (GameLogic, ObjectId) {
        let (mut world, id) = authored_with_extra(
            "NeutralStructureHeightOwner",
            "Behavior = HeightDieUpdate HeightTag\nTargetHeight = 0\nTargetHeightIncludesStructures = Yes\nEnd\nGeometry = SPHERE\nGeometryMajorRadius = 1\nGeometryHeight = 1",
            30,
            Vec3::new(0.0, 10.0, 0.0),
            &format!(
                "Object ReceivingStructure\nKindOf = STRUCTURE\nBody = ActiveBody StructureBodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nGeometry = CYLINDER\nGeometryIsSmall = No\nGeometryHeight = {height}\nGeometryMajorRadius = 0\nGeometryMinorRadius = 0\nEnd"
            ),
        );
        world
            .create_object("ReceivingStructure", Team::USA, Vec3::new(x, base_y, 0.0))
            .unwrap();
        (world, id)
    }
    // Source sphere radius1; tall cylinder centerY10/radius10. At x12 the
    // sphere-bound distance is exactly1, which C++ excludes with strict <.
    let (mut boundary, boundary_id) = scene(20.0, 12.0, 0.0);
    let (mut inside, inside_id) = scene(20.0, 11.5, 0.0);
    let (mut low, low_id) = scene(5.0, 0.0, 0.0);
    let (mut above, above_id) = scene(20.0, 0.0, 100.0);
    assert_eq!(boundary_id, inside_id);
    assert_eq!(inside_id, low_id);
    assert_eq!(low_id, above_id);
    drive(&mut inside, inside_id, 30);
    assert!(
        inside.host_object(inside_id).unwrap().status.on_die_started,
        "cylinder center offset admits the nearby tall structure"
    );
    for (world, id) in [
        (&mut boundary, boundary_id),
        (&mut low, low_id),
        (&mut above, above_id),
    ] {
        drive(world, id, 30);
        assert!(
            !world.host_object(id).unwrap().status.on_die_started,
            "peer geometry, strict boundary and vertically distant structures cannot influence this owner"
        );
        assert_eq!(world.host_object(id).unwrap().health.current, 100.0);
    }
}

#[test]
fn field_expiry_observes_earliest_timer_without_advancing_height_twice() {
    let (mut world, id) = authored(
        "NeutralFieldModuleOrder",
        "Behavior = LifetimeUpdate Late\nMinLifetime = 20000\nMaxLifetime = 20000\nEnd\nBehavior = LifetimeUpdate Early\nMinLifetime = 34\nMaxLifetime = 34\nEnd\nBehavior = HeightDieUpdate Height\nTargetHeight = 0\nInitialDelay = 34\nEnd",
        30,
        Vec3::new(0.0, 50.0, 0.0),
    );
    let object = world.host_object_mut(id).unwrap();
    object.anthrax_toxin_field = true;
    object.anthrax_toxin_field_expires_frame = Some(9999);
    object.indestructible = true;
    world.frame = 32;
    world.update_anthrax_toxin_field_objects().unwrap();
    let states = &world
        .host_object(id)
        .unwrap()
        .native_lifecycle
        .as_ref()
        .unwrap()
        .states;
    assert!(matches!(
        states[0],
        NativeLifecycleState::Lifetime { active: true, .. }
    ));
    assert!(matches!(
        states[1],
        NativeLifecycleState::Lifetime { active: false, .. }
    ));
    assert!(matches!(
        states[2],
        NativeLifecycleState::HeightDie {
            earliest_frame: u32::MAX,
            ..
        }
    ));
    world.tick_native_lifecycle(id).unwrap();
    let states = &world
        .host_object(id)
        .unwrap()
        .native_lifecycle
        .as_ref()
        .unwrap()
        .states;
    assert!(
        matches!(states[2],NativeLifecycleState::HeightDie{earliest_frame:34,last_position,..} if last_position==Vec3::splat(-1.0))
    );
    assert_eq!(world.host_object(id).unwrap().health.current, 100.0);
}

#[test]
fn authored_timer_runs_through_real_ai_without_a_name_profile() {
    let (mut world, id) = authored(
        "NeutralAiLifetime",
        "Behavior = LifetimeUpdate Clock\nMinLifetime = 34\nMaxLifetime = 34\nEnd",
        30,
        Vec3::ZERO,
    );
    world.frame = 31;
    world
        .update_ai(&[id], 1.0 / 30.0)
        .expect("receiving AI operation succeeds");
    assert_eq!(world.host_object(id).unwrap().health.current, 100.0);
    world.frame = 32;
    world
        .update_ai(&[id], 1.0 / 30.0)
        .expect("receiving AI operation succeeds");
    assert!(world.host_object(id).unwrap().status.on_die_started);
    assert_eq!(world.host_object(id).unwrap().health.current, 0.0);
}

#[test]
fn disabled_modules_defer_without_consuming_timer_or_height_delay() {
    for kind in ["LifetimeUpdate", "DeletionUpdate"] {
        let (mut world, id) = authored(
            "DisabledNativeTimer",
            &format!("Behavior = {kind} Clock\nMinLifetime = 34\nMaxLifetime = 34\nEnd"),
            30,
            Vec3::ZERO,
        );
        world.host_object_mut(id).unwrap().status.disabled_emp = true;
        drive(&mut world, id, 40);
        let object = world.host_object(id).unwrap();
        assert_eq!(object.health.current, 100.0);
        assert!(!object.status.destroyed);
        assert!(matches!(
            object.native_lifecycle.as_ref().unwrap().states[0],
            NativeLifecycleState::Lifetime { active: true, .. }
                | NativeLifecycleState::Deletion { active: true, .. }
        ));
        world.host_object_mut(id).unwrap().status.disabled_emp = false;
        drive(&mut world, id, 41);
        assert!(world.host_object(id).unwrap().status.destroyed);
    }
    let (mut world, id) = authored(
        "DisabledNativeHeight",
        "Behavior = HeightDieUpdate Height\nTargetHeight = 10\nInitialDelay = 34\nEnd",
        30,
        Vec3::new(0.0, 5.0, 0.0),
    );
    world.host_object_mut(id).unwrap().status.disabled_held = true;
    drive(&mut world, id, 40);
    assert!(matches!(
        world
            .host_object(id)
            .unwrap()
            .native_lifecycle
            .as_ref()
            .unwrap()
            .states[0],
        NativeLifecycleState::HeightDie {
            earliest_frame: u32::MAX,
            ..
        }
    ));
    world.host_object_mut(id).unwrap().status.disabled_held = false;
    drive(&mut world, id, 41);
    drive(&mut world, id, 42);
    assert_eq!(world.host_object(id).unwrap().health.current, 100.0);
    drive(&mut world, id, 43);
    assert_eq!(world.host_object(id).unwrap().health.current, 0.0);
}

#[test]
fn construction_and_weapon_jamming_are_not_disabled_types_for_lifecycle() {
    let (mut world, id) = authored(
        "ConstructionTimer",
        "Behavior = LifetimeUpdate Clock\nMinLifetime = 34\nMaxLifetime = 34\nEnd",
        30,
        Vec3::ZERO,
    );
    let object = world.host_object_mut(id).unwrap();
    object.status.under_construction = true;
    object.status.weapons_jammed = true;
    assert!(!object.has_disabled_type());
    drive(&mut world, id, 32);
    assert!(world.host_object(id).unwrap().status.on_die_started);
}

#[test]
fn mounted_crusader_death_notifies_the_gla_salvager_of_its_authored_crate_once() {
    use crate::game_logic::{
        combat::DamageType, host_usa_pilot::HostDeathType, object::DamageHitContext,
    };
    use game_engine::common::game_common::Relationship;

    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "mounted_crusader_death_notifies_the_gla_salvager_of_its_authored_crate_once",
        || {
            let install = std::env::var_os("GENERALS_RETAIL_CONTENT_ROOT")
                .map(std::path::PathBuf::from)
                .or_else(crate::map_frame_scenario::find_lone_eagle_retail_install)
                .expect("a real mounted retail install is required");
            let startup =
                crate::parallel_harness::RetailWorldDefinitions::load_from_install(Some(&install))
                    .unwrap_or_else(|error| panic!("{}: {error}", install.display()));
            let mut world = GameLogic::new();
            startup.admit(&mut world).unwrap();
            let mut usa = Player::new(1, Team::USA, "CrateVictim", true);
            usa.set_map_relationship(2, Relationship::Enemies);
            let mut gla = Player::new(2, Team::GLA, "CrateKiller", false);
            gla.set_map_relationship(1, Relationship::Enemies);
            assert!(gla.unlock_science("SCIENCE_GLA"));
            world.add_player(usa);
            world.add_player(gla);
            world.frame = 40;

            let origin = Vec3::new(180., 12., 220.);
            let victim = world
                .create_object_for_player_with_effects_checked(
                    "AmericaTankCrusader", 1, origin, None, &mut Default::default(),
                )
                .unwrap()
                .expect("stock USA Crusader");
            let killer = world
                .create_object_for_player_with_effects_checked(
                    "GLATankScorpion", 2, origin + Vec3::X * 35., None, &mut Default::default(),
                )
                .unwrap()
                .expect("stock GLA Scorpion");
            // A victim on a non-ground pathfinder layer drops at its own
            // position and layer (CreateCrateDie.cpp:187–195).
            world.set_owned_pathfinder_layer(victim, 2);
            let origin = world.host_object(victim).unwrap().get_position();

            world
                .apply_owned_damage_with_effects(
                    victim,
                    10_000.,
                    Some(killer),
                    DamageType::Unresistable,
                    HostDeathType::Normal,
                    None,
                    &DamageHitContext::default(),
                    &mut Default::default(),
                )
                .unwrap()
                .expect("lethal receiving Body application");

            assert!(world.host_object(victim).unwrap().status.on_die_started);
            let crate_id = world.host_object(killer).unwrap().crate_created
                .expect("native Die callback immediately informs the GLA computer unit");
            let crate_object = world.host_object(crate_id).expect("real receiving crate object");
            assert_eq!(crate_object.template_name, "SalvageCrate");
            assert_eq!(crate_object.team, Team::Neutral);
            assert_eq!(crate_object.get_position(), origin);
            assert_eq!(crate_object.pathfind_layer, 2);
            assert!(matches!(
                crate_object.native_lifecycle.as_ref().unwrap().states.as_ref(),
                [NativeLifecycleState::Deletion { die_frame, active: true }]
                    if (940..=1090).contains(die_frame)
            ));
            assert_eq!(
                crate_object.terrain_decal_type,
                crate::game_logic::host_battlemaster::TERRAIN_DECAL_CRATE
            );

            world.destroy_object(victim).unwrap();
            world.process_destroy_list();
            assert!(world.host_object(victim).is_none());
            assert_eq!(
                world.objects.registered().filter(|(_, object)| {
                    object.template_name == "SalvageCrate"
                }).map(|(id, _)| *id).collect::<Vec<_>>(),
                [crate_id]
            );
        },
    );
}

/// FireOCLAfterWeaponCooldownUpdate.cpp:78-172 — the authored module counts
/// consecutive shots on its configured slot through the per-object native
/// lifecycle, and runs its OCL when the weapon could have fired but did not.
/// The fixture renames the sprayer so nothing can be peeled from a template
/// name, and the stock toxin-truck name carries no module at all.
#[test]
fn authored_fire_ocl_after_cooldown_counts_slot_shots_through_snapshot() {
    use crate::assets::ini_template_loader::{
        WeaponDefinitionLoadType, parse_weapon_definitions_ini_into,
    };
    use crate::game_logic::game_logic::accepted_projectile_owner_tests::patriot_retail_battery_tests::{
        base_ini, definition,
    };
    use crate::save_load::snapshot::SnapshotBuilder;

    const SPRAYER: &str = "SprayUnitRenamed";
    const LOOKALIKE: &str = "GLAVehicleToxinTruck";
    const CHILD_A: &str = "SprayFieldChildA";
    const CHILD_B: &str = "SprayFieldChildB";
    const GATE: &str = "Upgrade_SprayFieldGate";
    const PRIMARY_WEAPON: &str = "ToxinTruckGun";
    const SECONDARY_WEAPON: &str = "ToxinTruckSprayer";

    fn fixture() -> (GameLogic, ObjectId, ObjectId) {
        let mut world = GameLogic::new();
        world.add_player(Player::new(0, Team::USA, "Owner", true));
        world.add_player(Player::new(1, Team::GLA, "Peer", false));
        world.frame = 100;
        world
            .admit_object_creation_list_sources([
                r#"
ObjectCreationList OCL_SprayFieldA
  CreateObject
    ObjectNames = SprayFieldChildA
    Count = 1
  End
End
ObjectCreationList OCL_SprayFieldB
  CreateObject
    ObjectNames = SprayFieldChildB
    Count = 1
  End
End
"#
                .to_string(),
            ])
            .unwrap();
        world
            .admit_object_ini_sources([(
                "fire-ocl-sprayer.ini".to_string(),
                format!(
                    r#"
Object {SPRAYER}
  KindOf = IMMOBILE INERT NO_COLLIDE
  Body = ActiveBody BodyTag
    MaxHealth = 500
    InitialHealth = 500
  End
  WeaponSet
    Conditions = None
    Weapon = PRIMARY {PRIMARY_WEAPON}
    Weapon = SECONDARY {SECONDARY_WEAPON}
  End
  Behavior = FireOCLAfterWeaponCooldownUpdate SprayCooldownA
    WeaponSlot = SECONDARY
    OCL = OCL_SprayFieldA
    MinShotsToCreateOCL = 2
    OCLLifetimePerSecond = 1000
    OCLLifetimeMaxCap = 100
  End
  Behavior = FireOCLAfterWeaponCooldownUpdate SprayCooldownB
    WeaponSlot = SECONDARY
    OCL = OCL_SprayFieldB
    MinShotsToCreateOCL = 3
    OCLLifetimePerSecond = 1000
    OCLLifetimeMaxCap = 100
    TriggeredBy = {GATE}
  End
End
Object {CHILD_A}
  KindOf = IMMOBILE INERT NO_COLLIDE
  Body = ActiveBody BodyTag
    MaxHealth = 10
    InitialHealth = 10
  End
  Behavior = LifetimeUpdate LifetimeTag
    MinLifetime = 9000
    MaxLifetime = 9000
  End
End
Object {CHILD_B}
  KindOf = IMMOBILE INERT NO_COLLIDE
  Body = ActiveBody BodyTag
    MaxHealth = 10
    InitialHealth = 10
  End
  Behavior = LifetimeUpdate LifetimeTag
    MinLifetime = 9000
    MaxLifetime = 9000
  End
End
Object {LOOKALIKE}
  KindOf = VEHICLE
  Body = ActiveBody BodyTag
    MaxHealth = 500
    InitialHealth = 500
  End
End
"#
                ),
            )])
            .unwrap();
        let weapon_source = base_ini("Weapon.ini");
        let mut weapons = std::collections::HashMap::new();
        for name in [PRIMARY_WEAPON, SECONDARY_WEAPON] {
            let source = definition(&weapon_source, &format!("Weapon {name}"));
            assert_eq!(
                parse_weapon_definitions_ini_into(
                    &mut weapons,
                    &source,
                    WeaponDefinitionLoadType::Normal,
                ),
                1
            );
        }
        world
            .admit_weapon_content_snapshot(
                weapons
                    .into_values()
                    .map(|rule| (rule.name.clone(), rule))
                    .collect(),
                &[],
            )
            .unwrap();
        let sprayer = world
            .create_object_for_player(SPRAYER, 0, Vec3::ZERO)
            .unwrap();
        let lookalike = world
            .create_object_for_player(LOOKALIKE, 0, Vec3::new(50.0, 0.0, 0.0))
            .unwrap();
        (world, sprayer, lookalike)
    }
    fn shot(world: &mut GameLogic, id: ObjectId, frame: u32) {
        world
            .host_object_mut(id)
            .unwrap()
            .weapon_slot_mut(1)
            .unwrap()
            .last_fire_frame = frame - 1;
        drive(world, id, frame);
    }
    fn set_ready(world: &mut GameLogic, id: ObjectId, next_frame: u32) {
        world
            .host_object_mut(id)
            .unwrap()
            .weapon_slot_mut(1)
            .unwrap()
            .next_fire_frame = next_frame;
    }
    fn count(world: &GameLogic, name: &str) -> usize {
        world
            .host_objects()
            .registered_values()
            .filter(|object| object.template_name == name)
            .count()
    }
    fn cooldown(world: &GameLogic, id: ObjectId, index: usize) -> (bool, u32) {
        match &world
            .host_object(id)
            .unwrap()
            .native_lifecycle
            .as_ref()
            .unwrap()
            .states[index]
        {
            NativeLifecycleState::FireOclAfterCooldown {
                valid,
                consecutive_shots,
                ..
            } => (*valid, *consecutive_shots),
            state => panic!("expected FireOclAfterCooldown state, got {state:?}"),
        }
    }

    let (mut world, sprayer, lookalike) = fixture();
    // Both authored modules bound in source order on the renamed sprayer; the
    // stock-name lookalike carries no module, no secondary weapon and no state.
    {
        let runtime = world
            .host_object(sprayer)
            .unwrap()
            .native_lifecycle
            .as_ref()
            .unwrap();
        assert_eq!(runtime.states.len(), 2);
        for (index, (min_shots, ocl)) in
            [(0usize, (2u32, "OCL_SprayFieldA")), (1, (3, "OCL_SprayFieldB"))]
        {
            match &runtime.definitions[index].kind {
                crate::game_logic::native_lifecycle_definition::NativeLifecycleKind::FireOclAfterCooldown(definition) => {
                    assert_eq!(definition.slot, 1);
                    assert_eq!(definition.min_shots, min_shots);
                    assert_eq!(definition.ocl.as_deref(), Some(ocl));
                    // 100ms authored cap -> 3 logic frames.
                    assert_eq!(definition.max_frames, 3);
                }
                kind => panic!("expected FireOclAfterCooldown definition, got {kind:?}"),
            }
        }
    }
    let lookalike_object = world.host_object(lookalike).unwrap();
    assert!(
        lookalike_object
            .native_lifecycle
            .as_ref()
            .is_none_or(|runtime| runtime.states.is_empty())
    );
    assert!(lookalike_object.weapon_slot(1).is_none());

    // The configured slot is SECONDARY; select it so the modules are valid.
    world
        .host_object_mut(sprayer)
        .unwrap()
        .set_active_weapon_slot(1);
    assert_eq!(
        world.host_object(sprayer).unwrap().selected_weapon_slot(),
        Some(1)
    );
    // The gated module stays invalid until the player upgrade completes.
    assert_eq!(cooldown(&world, sprayer, 0), (false, 0));
    assert_eq!(cooldown(&world, sprayer, 1), (false, 0));

    // Accumulate: each driven frame where the weapon fired the previous frame
    // adds one consecutive shot. The first shot lands on the invalid->valid
    // transition, and the C++ resetStats that follows the per-frame logic
    // consumes it.
    shot(&mut world, sprayer, 101);
    assert_eq!(cooldown(&world, sprayer, 0), (true, 0));
    assert_eq!(cooldown(&world, sprayer, 1), (false, 0));
    shot(&mut world, sprayer, 102);
    assert_eq!(cooldown(&world, sprayer, 0), (true, 1));

    // Round-trip while accumulating: streak and start frame persist.
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&world).unwrap();
    let mut loaded = fixture().0;
    builder.restore_from_snapshot(&snapshot, &mut loaded).unwrap();
    assert_eq!(cooldown(&loaded, sprayer, 0), (true, 1));
    let mut world = loaded;

    shot(&mut world, sprayer, 103);
    shot(&mut world, sprayer, 104);
    shot(&mut world, sprayer, 105);
    assert_eq!(cooldown(&world, sprayer, 0), (true, 4));
    assert_eq!(cooldown(&world, sprayer, 1), (false, 0));
    assert_eq!(count(&world, CHILD_A), 0);

    // Exact deadline equality does not emit; the first frame strictly past
    // the committed deadline does (Weapon.cpp possible-next-shot check).
    set_ready(&mut world, sprayer, 106);
    drive(&mut world, sprayer, 106);
    assert_eq!(count(&world, CHILD_A), 0);
    assert_eq!(cooldown(&world, sprayer, 0), (true, 4));
    drive(&mut world, sprayer, 107);
    assert_eq!(count(&world, CHILD_A), 1);
    assert_eq!(count(&world, CHILD_B), 0);
    // Streak is consumed by the fire, and the child carries producer identity
    // plus the caller lifetime capped at the authored 3 frames.
    assert_eq!(cooldown(&world, sprayer, 0), (true, 0));
    {
        let child = world
            .host_objects()
            .registered_values()
            .find(|object| object.template_name == CHILD_A)
            .unwrap();
        assert_eq!(child.producer_id, Some(sprayer));
        match &child.native_lifecycle.as_ref().unwrap().states[0] {
            NativeLifecycleState::Lifetime { die_frame, .. } => {
                // Raw streak 5 frames (107-102) is capped at 3 -> 107 + 3.
                assert_eq!(*die_frame, 110);
            }
            state => panic!("expected Lifetime state on the OCL child, got {state:?}"),
        }
    }
    // No repeated emission once the streak is consumed.
    drive(&mut world, sprayer, 108);
    drive(&mut world, sprayer, 109);
    assert_eq!(count(&world, CHILD_A), 1);

    // Weapon switch away from the configured slot flushes the accumulated
    // streak while the watched slot still has its weapon.
    shot(&mut world, sprayer, 110);
    shot(&mut world, sprayer, 111);
    world
        .host_object_mut(sprayer)
        .unwrap()
        .set_active_weapon_slot(0);
    drive(&mut world, sprayer, 112);
    assert_eq!(count(&world, CHILD_A), 2);
    assert_eq!(count(&world, CHILD_B), 0);
    // Switching back re-validates the module; the streak restarts.
    world
        .host_object_mut(sprayer)
        .unwrap()
        .set_active_weapon_slot(1);
    drive(&mut world, sprayer, 113);
    assert_eq!(cooldown(&world, sprayer, 0), (true, 0));

    // The gated module becomes valid once the controlling player completes
    // the authored trigger upgrade.
    world.add_completed_player_upgrade(0, GATE);
    drive(&mut world, sprayer, 114);
    assert_eq!(cooldown(&world, sprayer, 1), (true, 0));
    shot(&mut world, sprayer, 115);
    shot(&mut world, sprayer, 116);
    // Module A fires at its own threshold; B's larger threshold holds.
    drive(&mut world, sprayer, 117);
    assert_eq!(count(&world, CHILD_A), 3);
    assert_eq!(count(&world, CHILD_B), 0);
    assert_eq!(cooldown(&world, sprayer, 1), (true, 2));
    shot(&mut world, sprayer, 118);
    drive(&mut world, sprayer, 119);
    assert_eq!(count(&world, CHILD_A), 3);
    assert_eq!(count(&world, CHILD_B), 1);
    {
        let child = world
            .host_objects()
            .registered_values()
            .find(|object| object.template_name == CHILD_B)
            .unwrap();
        assert_eq!(child.producer_id, Some(sprayer));
        match &child.native_lifecycle.as_ref().unwrap().states[0] {
            NativeLifecycleState::Lifetime { die_frame, .. } => {
                assert_eq!(*die_frame, 122);
            }
            state => panic!("expected Lifetime state on the gated OCL child, got {state:?}"),
        }
    }
    drive(&mut world, sprayer, 120);
    drive(&mut world, sprayer, 121);
    assert_eq!(count(&world, CHILD_B), 1);

    // The stock-name lookalike never accumulates or emits without the module.
    drive(&mut world, lookalike, 122);
    drive(&mut world, lookalike, 123);
    assert_eq!(count(&world, CHILD_A), 3);
    assert_eq!(count(&world, CHILD_B), 1);
}
