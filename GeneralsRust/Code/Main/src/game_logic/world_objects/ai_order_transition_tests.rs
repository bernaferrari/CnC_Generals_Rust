//! Receiving AI orders and their published targets, movement, and guard state.

use crate::game_logic::{AICommand, AIState, GameLogic, KindOf, ObjectId, Team, ThingTemplate};
use crate::presentation_frame::{PresentationFrame, RenderableObject};
use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
use glam::Vec3;

fn publish(world: &mut GameLogic) -> PresentationFrame {
    let local = world.local_player_id().unwrap_or(0);
    PresentationFrame::try_build_from_logic(world, local).expect("publish receiving world")
}

fn actor(frame: &PresentationFrame, id: ObjectId) -> &RenderableObject {
    frame
        .objects
        .iter()
        .find(|row| row.id == id)
        .expect("published actor")
}

fn insert_fixture_template(logic: &mut GameLogic, name: &str, hp: f32) {
    let mut template = ThingTemplate::new(name);
    template.set_health(hp);
    template.add_kind_of(KindOf::Selectable);
    template.add_kind_of(KindOf::Attackable);
    logic.templates.insert(name.into(), template);
}

#[test]
fn move_order_keeps_raw_ai_goal_and_publishes_installed_destination() {
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("AiDec");
    crate::fixture_content::admit_fixture_content(
        &mut logic,
        &[crate::fixture_content::FixtureObject::new("AdU")
            .kind_of("INFANTRY")
            .health(100.0)
            .walker()
            .armed("OrderFixtureGun")],
        &[
            crate::fixture_content::FixtureWeapon::new("OrderFixtureGun")
                .damage(10.0)
                .range(100.0),
        ],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let oid = logic
        .create_object("AdU", Team::USA, Vec3::new(5.0, 0.0, 5.0))
        .expect("id");
    let vid = logic
        .create_object("AdU", Team::China, Vec3::new(25.0, 0.0, 5.0))
        .expect("v");
    logic.apply_ai_command_for_test(crate::game_logic::game_logic::AICommand::AttackTarget {
        object_id: oid,
        target_id: vid,
    });
    logic.apply_ai_command_for_test(crate::game_logic::game_logic::AICommand::MoveTo {
        object_id: oid,
        position: Vec3::new(14.5, 0.0, 14.5),
    });
    let object = logic.host_object(oid).unwrap();
    // AIInternalMoveToState::onEnter (AIStates.cpp:1638-1645) adjusts only
    // the state's path goal; the machine goal keeps the raw AI destination.
    assert_eq!(
        object.capture_base_ai_continuation().goals.position,
        Vec3::new(14.5, 0.0, 14.5)
    );
    let path_goal = object.movement.target_position;
    assert!(path_goal.is_some());

    let frame = publish(&mut logic);
    assert_eq!(actor(&frame, oid).move_destination, path_goal);
}

#[test]
fn attack_command_sets_target_and_state_in_the_same_frame() {
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("AiCmdAuth");
    crate::fixture_content::admit_fixture_content(
        &mut logic,
        &[crate::fixture_content::FixtureObject::new("AcU")
            .kind_of("INFANTRY")
            .health(100.0)
            .walker()
            .armed("OrderFixtureGun")],
        &[
            crate::fixture_content::FixtureWeapon::new("OrderFixtureGun")
                .damage(10.0)
                .range(100.0),
        ],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let oid = logic
        .create_object("AcU", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    let vid = logic
        .create_object("AcU", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
        .expect("v");
    logic.apply_ai_command_for_test(AICommand::AttackTarget {
        object_id: oid,
        target_id: vid,
    });
    logic.apply_ai_command_for_test(AICommand::SetAIState {
        object_id: oid,
        state: crate::game_logic::AIState::Attacking,
    });

    let host = logic.host_objects().lookup(&oid).unwrap();
    assert_eq!(
        host.target,
        Some(vid),
        "host applies AttackTarget same-frame"
    );
    assert_eq!(
        host.ai_state(),
        crate::game_logic::AIState::Attacking,
        "host applies SetAIState same-frame"
    );

    let frame = publish(&mut logic);
    let published = actor(&frame, oid);
    assert_eq!(published.attack_target, Some(vid));
    assert_eq!(published.ai_state_ordinal, AIState::Attacking.to_ordinal());
}

#[test]
fn attack_continuation_installs_the_next_victim() {
    // C++ loads Object and Weapon INI before NewGame, with the PlayerTemplate
    // catalog the skirmish slots resolve against.
    let mut logic =
        crate::skirmish_config::lobby_definition_tests::slot_world_with_objects_and_weapons(
            &[
                crate::fixture_content::FixtureObject::new("CaA")
                    .kind_of("INFANTRY")
                    .health(100.0)
                    .walker()
                    .armed("OrderFixtureGun"),
                crate::fixture_content::FixtureObject::new("CaD")
                    .kind_of("INFANTRY")
                    .health(100.0)
                    .walker()
                    .armed("OrderFixtureGun"),
                crate::fixture_content::FixtureObject::new("CaN")
                    .kind_of("INFANTRY")
                    .health(100.0)
                    .walker()
                    .armed("OrderFixtureGun"),
            ],
            &[
                crate::fixture_content::FixtureWeapon::new("OrderFixtureGun")
                    .damage(10.0)
                    .range(100.0),
            ],
        );
    let cfg = crate::skirmish_config::lobby_definition_tests::slot_config(
        &logic,
        &golden_skirmish_config("ContAtk"),
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let attacker = logic
        .create_object("CaA", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("a");
    let dead = logic
        .create_object("CaD", Team::GLA, Vec3::new(5.0, 0.0, 0.0))
        .expect("d");
    let next = logic
        .create_object("CaN", Team::GLA, Vec3::new(8.0, 0.0, 0.0))
        .expect("n");
    // PartitionManager::update runs each frame; the new Objects are in its
    // cells before any radius query.
    logic
        .update_owned_partition_with_effects(&mut Default::default())
        .unwrap();
    // No vision pass runs in this raw fixture; the next victim is in front of
    // the attacker, i.e. shroud-clear for its owning player (canAttack gate).
    if let Some(pid) = logic.player_id_for_team(Team::USA) {
        let mut shroud = logic
            .world_services
            .shroud()
            .lock()
            .expect("fixture world shroud must be available");
        for victim in [dead, next] {
            shroud.set_host_object_shroud_status(
                pid,
                victim.0,
                gamelogic::common::ObjectShroudStatus::Clear,
            );
        }
    }
    let dead_pos = Vec3::new(5.0, 0.0, 0.0);
    let ok =
        logic.try_continue_attack_after_kill_for_test(attacker, dead, dead_pos, 50.0, Team::GLA);
    assert!(ok, "must find next victim in continue range");

    assert_eq!(
        logic.host_object(attacker).unwrap().target,
        Some(next),
        "original continuation installs its next victim immediately"
    );
    assert_eq!(
        actor(&publish(&mut logic), attacker).attack_target,
        Some(next)
    );
}

#[test]
fn attack_path_installs_and_publishes_its_victim() {
    use crate::game_logic::Weapon;

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("AtkPath");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[
            crate::fixture_content::FixtureObject::new("ApU")
                .kind_of("INFANTRY")
                .health(100.0)
                .walker(),
            crate::fixture_content::FixtureObject::new("ApE")
                .kind_of("INFANTRY")
                .health(100.0)
                .walker(),
        ],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let uid = logic
        .create_object("ApU", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("u");
    let vid = logic
        .create_object("ApE", Team::GLA, Vec3::new(80.0, 0.0, 0.0))
        .expect("e");
    if let Some(o) = logic.host_object_mut(uid) {
        crate::fixture_content::arm_slot_with_stats(
            o,
            0,
            Weapon {
                damage: 10.0,
                range: 25.0,
                ..Weapon::default()
            },
        );
    }
    let tpos = Vec3::new(80.0, 0.0, 0.0);
    let ok = logic.assign_unit_attack_path_for_test(uid, Some(vid), tpos);
    assert!(ok, "attack path should assign");

    let host = logic.host_objects().lookup(&uid).unwrap();
    assert_eq!(
        host.target,
        Some(vid),
        "original path assignment installs its victim immediately"
    );
    assert!(
        !host.movement.path.is_empty() || host.movement.target_position.is_some(),
        "path must still be assigned on host"
    );

    let path_goal = host.movement.target_position;
    let path_len = host.movement.path.len();
    let frame = publish(&mut logic);
    let published = actor(&frame, uid);
    assert_eq!(published.attack_target, Some(vid));
    assert_eq!(published.move_destination, path_goal);
    assert_eq!(usize::from(published.path_len), path_len);
}

#[test]
fn path_approach_preserves_the_requested_state() {
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("PathSt");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object PsU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("PsU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("PsU".into(), t);
    }
    let oid = logic
        .create_object("PsU", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    logic.path_approach_with_state_for_test(oid, Vec3::new(40.0, 0.0, 0.0), AIState::Gathering);

    assert_eq!(
        logic.host_object(oid).unwrap().ai_state(),
        AIState::Gathering
    );
    assert_eq!(
        actor(&publish(&mut logic), oid).ai_state_ordinal,
        AIState::Gathering.to_ordinal()
    );
}

#[test]
fn troop_crawler_assault_engages_from_the_receiving_world() {
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("TcAuth");
    crate::fixture_content::admit_fixture_content(
        &mut logic,
        &[
            crate::fixture_content::FixtureObject::new("ChinaVehicleTroopCrawler")
                .kind_of("VEHICLE")
                .health(100.0)
                .walker(),
            crate::fixture_content::FixtureObject::new("TcO")
                .kind_of("INFANTRY")
                .health(100.0)
                .walker()
                .armed("OrderFixtureGun"),
            crate::fixture_content::FixtureObject::new("TcE")
                .kind_of("INFANTRY")
                .health(100.0)
                .walker()
                .armed("OrderFixtureGun"),
        ],
        &[
            crate::fixture_content::FixtureWeapon::new("OrderFixtureGun")
                .damage(10.0)
                .range(100.0),
        ],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let crawler_name = "ChinaVehicleTroopCrawler";
    let crawler = logic
        .create_object(crawler_name, Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("c");
    let occ = logic
        .create_object("TcO", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .expect("o");
    let enemy = logic
        .create_object("TcE", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
        .expect("e");
    if let Some(c) = logic.host_object_mut(crawler) {
        c.install_troop_crawler_transport();
        let _ = c.add_occupant(occ);
    }
    if let Some((o, fixture_partition)) = logic.host_object_and_partition_mut(occ) {
        o.set_contained_by_with_partition_owner(Some(crawler), true, fixture_partition);
    }
    let ordered = logic.apply_troop_crawler_assault_deploy_for_test(crawler, enemy);
    assert!(
        ordered >= 1,
        "deploy should order occupant attack; ordered={ordered}"
    );

    let host_engaged = logic
        .host_objects()
        .registered()
        .any(|(id, o)| *id != enemy && o.target == Some(enemy));
    assert!(
        host_engaged,
        "assault deploy must set host target same-frame; ordered={ordered} occ_target={:?}",
        logic.host_objects().lookup(&occ).map(|o| o.target)
    );

    let frame = publish(&mut logic);
    assert!(
        frame
            .objects
            .iter()
            .any(|row| row.id != enemy && row.attack_target == Some(enemy))
    );
}

#[test]
fn laser_guided_preparation_engages_with_the_secondary_weapon() {
    use crate::game_logic::Weapon;

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("MdAuth");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[
            crate::fixture_content::FixtureObject::new("AmericaInfantryMissileDefender")
                .kind_of("INFANTRY")
                .health(100.0)
                .walker(),
            crate::fixture_content::FixtureObject::new("MdE")
                .kind_of("INFANTRY")
                .health(100.0)
                .walker(),
        ],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let md_name = "AmericaInfantryMissileDefender";
    let mid = logic
        .create_object(md_name, Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("md");
    let eid = logic
        .create_object("MdE", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("e");
    if let Some(o) = logic.host_object_mut(mid) {
        crate::fixture_content::arm_slot_with_stats(
            o,
            1,
            Weapon {
                damage: 20.0,
                range: 250.0,
                ..Weapon::default()
            },
        );
        crate::fixture_content::arm_slot_with_stats(
            o,
            0,
            Weapon {
                damage: 5.0,
                range: 100.0,
                ..Weapon::default()
            },
        );
    }
    let ok = logic.activate_missile_defender_laser_guided_for_test(mid, eid);
    assert!(ok, "laser guided should activate");
    // C++ SpecialAbilityUpdate advances startPreparation → continuePreparation
    // → triggerAbilityEffect on later updates (SpecialAbilityUpdate.cpp
    // :1276-1293). Drive the actual preparation until it installs the
    // secondary-weapon engagement on the canonical object.
    for _ in 0..600 {
        if logic.host_object(mid).is_some_and(|object| {
            object.target == Some(eid)
                && object.ai_state() == crate::game_logic::AIState::Attacking
                && object.active_weapon_slot == 1
        }) {
            break;
        }
        logic.update_support_states_for_test(&[mid, eid], 1.0 / 30.0);
    }

    assert_eq!(
        logic.host_objects().lookup(&mid).unwrap().target,
        Some(eid),
        "completed preparation installs the target"
    );
    assert_eq!(
        logic
            .host_objects()
            .lookup(&mid)
            .unwrap()
            .active_weapon_slot,
        1
    );

    let frame = publish(&mut logic);
    let published = actor(&frame, mid);
    assert_eq!(published.attack_target, Some(eid));
    assert_eq!(published.ai_state_ordinal, AIState::Attacking.to_ordinal());
    assert_eq!(published.active_weapon_slot, 1);
}

#[test]
fn private_attack_object_publishes_the_actor_target() {
    use crate::game_logic::Weapon;

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("PrivAtk");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[
            crate::fixture_content::FixtureObject::new("PaU")
                .kind_of("INFANTRY")
                .health(100.0)
                .walker(),
            crate::fixture_content::FixtureObject::new("PaE")
                .kind_of("INFANTRY")
                .health(100.0)
                .walker(),
        ],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let uid = logic
        .create_object("PaU", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("u");
    let vid = logic
        .create_object("PaE", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("e");
    if let Some(o) = logic.host_object_mut(uid) {
        crate::fixture_content::arm_slot_with_stats(
            o,
            0,
            Weapon {
                damage: 10.0,
                range: 50.0,
                ..Weapon::default()
            },
        );
    }
    let ok = logic.private_attack_object_for_test(uid, vid, -1);
    assert!(ok, "private_attack_object should enter attack SM");

    // AIUpdate.cpp:3395 installs the goal before entering the attack state.
    // Diagnostic decision recording does not postpone the host transition.
    assert_eq!(logic.host_objects().lookup(&uid).unwrap().target, Some(vid));
    assert_eq!(actor(&publish(&mut logic), uid).attack_target, Some(vid));
}

#[test]
fn transfer_attack_retargets_the_original_attacker() {
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("XferAtk");
    logic
        .admit_object_ini_sources([(
            "fixture-objects.ini".into(),
            "Object XaA\nEnd\nObject XaFrom\nEnd\nObject XaTo\nEnd\n".into(),
        )])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for name in ["XaA", "XaFrom", "XaTo"] {
        {
            let mut t = ThingTemplate::new(name);
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Attackable);
            logic.templates.insert(name.into(), t);
        }
    }
    let attacker = logic
        .create_object("XaA", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("a");
    let from = logic
        .create_object("XaFrom", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .expect("from");
    let to = logic
        .create_object("XaTo", Team::GLA, Vec3::new(12.0, 0.0, 0.0))
        .expect("to");
    // Seed host engagement on destroyed/old victim.
    if let Some(o) = logic.host_object_mut(attacker) {
        o.target = Some(from);
        o.status.attacking = true;
    }
    let before = publish(&mut logic);
    assert_eq!(actor(&before, attacker).attack_target, Some(from));
    assert_eq!(logic.transfer_attack(from, to), 1);

    assert_eq!(
        logic.host_objects().lookup(&attacker).unwrap().target,
        Some(to),
        "host must retarget immediately"
    );

    assert_eq!(
        actor(&publish(&mut logic), attacker).attack_target,
        Some(to)
    );
    assert_eq!(actor(&before, attacker).attack_target, Some(from));
}

#[test]
fn engagement_publishes_the_selected_target() {
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("EngAw");
    crate::fixture_content::admit_fixture_content(
        &mut logic,
        &[
            crate::fixture_content::FixtureObject::new("EaU")
                .kind_of("INFANTRY")
                .health(100.0)
                .walker()
                .armed("OrderFixtureGun"),
            crate::fixture_content::FixtureObject::new("EaE")
                .kind_of("INFANTRY")
                .health(100.0)
                .walker()
                .armed("OrderFixtureGun"),
        ],
        &[
            crate::fixture_content::FixtureWeapon::new("OrderFixtureGun")
                .damage(10.0)
                .range(100.0),
        ],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let uid = logic
        .create_object("EaU", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("u");
    let vid = logic
        .create_object("EaE", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
        .expect("e");
    logic.apply_engagement_decision_aware_for_test(uid, vid);

    // AIAttackState::onEnter installs the victim (AIStates.cpp).
    assert_eq!(logic.host_objects().lookup(&uid).unwrap().target, Some(vid));
    assert_eq!(actor(&publish(&mut logic), uid).attack_target, Some(vid));
}

#[test]
fn idle_mood_publishes_the_auto_acquired_target() {
    // C++ loads Object and Weapon INI before NewGame, with the PlayerTemplate
    // catalog the skirmish slots resolve against.
    let mut logic =
        crate::skirmish_config::lobby_definition_tests::slot_world_with_objects_and_weapons(
            &[
                // Retail infantry author VisionRange (AmericaInfantry.ini Ranger 150);
                // the mood scan fails closed on vision <= 0.
                crate::fixture_content::FixtureObject::new("MaU")
                    .kind_of("INFANTRY")
                    .health(100.0)
                    .field("VisionRange = 150.0")
                    .walker()
                    .armed("MoodFixtureGun"),
            ],
            &[crate::fixture_content::FixtureWeapon::new("MoodFixtureGun")
                .damage(10.0)
                .range(100.0)],
        );
    logic.set_ai_definition_base(game_engine::common::ini::AIData {
        guard_outer_modifier_human: 1.333,
        guard_outer_modifier_ai: 2.2,
        aggressive_range_modifier: 1.5,
        ..Default::default()
    });
    let cfg = crate::skirmish_config::lobby_definition_tests::slot_config(
        &logic,
        &golden_skirmish_config("MoodAcq"),
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let oid = logic
        .create_object("MaU", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    let vid = logic
        .create_object("MaU", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
        .expect("v");
    {
        // C++ human idle auto-acquire only takes OBJECTSHROUD_CLEAR victims
        // (AIUpdate.cpp:4608-4619 PartitionFilterFreeOfFog residual, mood.rs
        // UNFOGGED flag for a local player's scan). No vision pass runs in this
        // raw fixture, so stamp the retail precondition: the enemy in front of
        // the start units is shroud-clear for the owning local player.
        if let Some(pid) = logic.player_id_for_team(Team::USA) {
            logic
                .world_services
                .shroud()
                .lock()
                .expect("fixture world shroud must be available")
                .set_host_object_shroud_status(
                    pid,
                    vid.0,
                    gamelogic::common::ObjectShroudStatus::Clear,
                );
        }
        let o = logic.host_object_mut(oid).expect("o");
        // Mood auto-acquire is an IDLE-state scan (mood.rs:601 eligible gate);
        // the try_mood_auto_acquire_enters_attack precedent sets Idle explicitly.
        o.set_ai_state(
            crate::game_logic::AIState::Idle,
            0,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        o.set_auto_acquire_when_idle(true);
        // C++ AutoAcquireEnemiesWhenIdle is a BITFIELD parsed from INI
        // (AIUpdate.cpp; AUTO_ACQUIRE_* flags) — get_next_mood_target gates
        // called_during_idle on AUTO_ACQUIRE_IDLE, not the legacy bool.
        o.auto_acquire_idle_bits |=
            game_engine::common::thing::ai_update_data::AUTO_ACQUIRE_IDLE;
        // AIIdleState::onEnter arms resetNextMoodCheckTime (now + forceIdleFrames),
        // and getNextMoodTarget returns NULL until then (AIUpdate.cpp:4543). The
        // victim arrives and the owner is nudged with
        // AIUpdateInterface::wakeUpAndAttemptToTarget (AIUpdate.cpp:4428).
        o.next_mood_check_time = 0;
        o.randomly_offset_mood_check = true;
    }
    // PartitionManager::update runs each frame; the new Objects are in its
    // cells before the mood scan queries them.
    logic
        .update_owned_partition_with_effects(&mut Default::default())
        .unwrap();
    // Drive one mood tick.
    logic.tick_mood_auto_acquire_for_test(&[oid]);

    // AIStates.cpp:1417-1427 enters the attack during the owning mood update.
    assert_eq!(logic.host_objects().lookup(&oid).unwrap().target, Some(vid));
    assert_eq!(actor(&publish(&mut logic), oid).attack_target, Some(vid));
}

#[test]
fn support_guard_publishes_its_engagement() {
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("GuardEng");
    crate::fixture_content::admit_fixture_content(
        &mut logic,
        &[crate::fixture_content::FixtureObject::new("GeU")
            .kind_of("INFANTRY")
            .health(100.0)
            .walker()
            .armed("OrderFixtureGun")],
        &[
            crate::fixture_content::FixtureWeapon::new("OrderFixtureGun")
                .damage(10.0)
                .range(100.0),
        ],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let oid = logic
        .create_object("GeU", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    let vid = logic
        .create_object("GeU", Team::GLA, Vec3::new(15.0, 0.0, 0.0))
        .expect("v");
    // Support states enter engagement through this command.
    logic.engage_target_decision_aware_for_test(oid, vid);

    assert_eq!(
        logic.host_objects().lookup(&oid).unwrap().target,
        Some(vid),
        "host engage immediate"
    );
    assert_eq!(actor(&publish(&mut logic), oid).attack_target, Some(vid));
}

#[test]
fn faction_ai_launch_publishes_its_attack() {
    use crate::ai::{AIDifficulty, AIPlayer};

    use crate::game_logic::Weapon;

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("FacAtk");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[
            crate::fixture_content::FixtureObject::new("FacU")
                .kind_of("INFANTRY")
                .health(100.0)
                .walker(),
            crate::fixture_content::FixtureObject::new("FacE")
                .kind_of("INFANTRY")
                .health(100.0)
                .walker(),
        ],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for (name, team, x) in [("FacU", Team::USA, 0.0f32), ("FacE", Team::GLA, 80.0)] {
        let _ = match logic.apply_host_object_id_op(crate::game_logic::HostObjectIdOp::Create {
            template: name.to_string(),
            team: team,
            spawn_at: Vec3::new(x, 0.0, 0.0),
        }) {
            crate::game_logic::HostObjectIdResult::Created(id) => id,
            _ => None,
        };
    }
    let usa_id = logic
        .get_players()
        .iter()
        .find(|(_, p)| p.team == Team::USA)
        .map(|(id, _)| *id)
        .unwrap_or(0);
    let gla_id = logic
        .get_players()
        .iter()
        .find(|(_, p)| p.team == Team::GLA)
        .map(|(id, _)| *id);
    let enemy = logic
        .host_objects()
        .registered()
        .find(|(_, o)| o.team == Team::GLA)
        .map(|(id, _)| *id)
        .expect("enemy");
    let usa_unit = logic
        .host_objects()
        .registered()
        .find(|(_, o)| o.team == Team::USA)
        .map(|(id, _)| *id)
        .expect("usa");
    if let Some(o) = logic.host_object_mut(usa_unit) {
        crate::fixture_content::arm_slot_with_stats(
            o,
            0,
            Weapon {
                damage: 10.0,
                ..Weapon::default()
            },
        );
    }
    let mut ai = AIPlayer::new(usa_id, Team::USA, AIDifficulty::Medium);
    ai.enemy_player_id = gla_id;
    ai.is_active = true;
    ai.launch_attack(&mut logic, 1000.0);

    assert_eq!(
        logic.host_objects().lookup(&usa_unit).unwrap().target,
        Some(enemy),
        "launch_attack must engage host target immediately"
    );
    assert_eq!(
        actor(&publish(&mut logic), usa_unit).attack_target,
        Some(enemy)
    );
}

#[test]
fn stop_attack_clears_the_current_target_without_rewriting_the_previous_frame() {
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("StopAtk");
    logic
        .admit_object_ini_sources([(
            "fixture-objects.ini".into(),
            "Object Su\nEnd\nObject Se\nEnd\n".into(),
        )])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for name in ["Su", "Se"] {
        {
            let mut t = ThingTemplate::new(name);
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Attackable);
            logic.templates.insert(name.into(), t);
        }
    }
    let oid = logic
        .create_object("Su", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("u");
    let vid = logic
        .create_object("Se", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .expect("e");
    // Seed host target as if previously engaged.
    if let Some(o) = logic.host_object_mut(oid) {
        o.target = Some(vid);
        o.status.attacking = true;
    }
    let before = publish(&mut logic);
    assert_eq!(actor(&before, oid).attack_target, Some(vid));
    logic.stop_attack_decision_aware_for_test(oid);

    // Host engagement clears same-frame so combat cannot keep firing.
    assert!(
        logic.host_objects().lookup(&oid).unwrap().target.is_none(),
        "host target must clear immediately on stop"
    );

    assert_eq!(actor(&publish(&mut logic), oid).attack_target, None);
    assert_eq!(actor(&before, oid).attack_target, Some(vid));
}

#[test]
fn assigning_a_path_enters_moving() {
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("AiPathOrder");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object AiUnit\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    insert_fixture_template(&mut logic, "AiUnit", 100.0);
    if let Some(t) = logic.templates.get_mut("AiUnit") {
        t.add_kind_of(KindOf::Infantry);
    }
    let id = logic
        .create_object("AiUnit", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("unit");
    if let Some(o) = logic.host_object_mut(id) {
        o.template_mut().add_kind_of(KindOf::Infantry);
        o.movement.max_speed = 30.0;
    }
    assert!(
        logic.assign_unit_path(id, Vec3::new(50.0, 0.0, 0.0), &[]),
        "assign_unit_path"
    );
    let st = logic.host_objects().lookup(&id).unwrap().ai_state().clone();
    assert!(
        matches!(st, crate::game_logic::AIState::Moving),
        "path assignment must enter Moving immediately, got {st:?}"
    );
}

#[test]
fn fixed_tick_advances_and_publishes_the_actor_pose() {
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("PublishMovement");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[
            crate::fixture_content::FixtureObject::new("VictoryKeepAlive")
                .kind_of("STRUCTURE MP_COUNT_FOR_VICTORY")
                .health(100.0),
            crate::fixture_content::FixtureObject::new("MvU")
                .kind_of("INFANTRY SELECTABLE")
                .health(100.0)
                .walker(),
        ],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    // update_with_dt evaluates the skirmish NO_BUILDINGS victory rule
    // (C++ VictoryConditions.cpp:87-95/168-196): a structure-less playable
    // player is defeated on frame 0-1. Seed the MpCountForVictory keep-alive
    // (stale_engine_id_does_not_skip_host_movement precedent).
    let _keep_alive = logic
        .create_object("VictoryKeepAlive", Team::USA, Vec3::new(300.0, 0.0, 300.0))
        .expect("keep-alive structure");
    let id = logic
        .create_object("MvU", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("unit");
    {
        let o = logic.host_object_mut(id).unwrap();
        // The stale_engine_id twin authors the resolved locomotor path
        // directly: a mapless first-tick assign still sits in the
        // computePointOnPath lead fallback (AIPathfind.cpp:910-950) where the
        // Locomotor.cpp:1393-1430 approach brake zeroes goalSpeed on
        // distAlongPath 0. Authoring the path mirrors the post-pathfind
        // FollowPath state the authority must integrate.
        o.movement
            .set_path_coords(vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(100.0, 0.0, 0.0)]);
        o.movement.max_speed = 60.0;
        // C++ Locomotor::getMaxAcceleration: retail locomotors author
        // Acceleration; velocity ramps by acceleration*dt — a 0-accel
        // fixture never moves.
        o.movement.acceleration = 240.0;
        o.movement.target_position = Some(Vec3::new(100.0, 0.0, 0.0));
        o.status.moving = true;
    }
    let before = publish(&mut logic);
    let pre = logic.host_object(id).unwrap().get_position();
    for _ in 0..10 {
        logic.update_with_dt(1.0 / 30.0);
    }
    let post = logic.host_objects().lookup(&id).unwrap().get_position();
    let dist = (post - pre).length();
    assert!(
        dist > 0.01,
        "fixed ticks must integrate the receiving path; pre={pre:?} post={post:?} dist={dist}"
    );

    let after = publish(&mut logic);
    assert_eq!(actor(&after, id).position, post);
    assert_eq!(actor(&before, id).position, pre);
}

#[test]
fn created_structure_starts_completed() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("CreatedStructure");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object HoleT\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    insert_fixture_template(&mut logic, "HoleT", 500.0);
    if let Some(t) = logic.templates.get_mut("HoleT") {
        t.add_kind_of(KindOf::Structure);
    }
    let id = logic
        .create_object("HoleT", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("hole");
    assert_eq!(logic.host_object(id).unwrap().construction_percent, 1.0);
    assert!(!logic.host_object(id).unwrap().status.under_construction);
}

#[test]
fn target_choice_publishes_the_exact_attack_receipt() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("AtkLog");
    logic
        .admit_object_ini_sources([(
            "fixture-objects.ini".into(),
            "Object LogA\nEnd\nObject LogB\nEnd\n".into(),
        )])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    insert_fixture_template(&mut logic, "LogA", 100.0);
    insert_fixture_template(&mut logic, "LogB", 100.0);
    let a = logic
        .create_object("LogA", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("a");
    let b = logic
        .create_object("LogB", Team::GLA, Vec3::new(15.0, 0.0, 0.0))
        .expect("b");
    if let Some((obj, attack_events)) = logic.host_object_and_attack_events_mut(a) {
        obj.set_target(
            Some(b),
            0,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }

    assert_eq!(logic.host_object(a).unwrap().target, Some(b));
    logic.complete_presentation_observations();
    let frame = publish(&mut logic);
    let receipts: Vec<_> = frame
        .events
        .iter()
        .filter_map(|event| match event {
            crate::presentation_frame::PresentationEvent::AttackTargeted { attacker, target } => {
                Some((*attacker, *target))
            }
            _ => None,
        })
        .collect();
    assert_eq!(receipts, [(a, Some(b))]);
    assert_eq!(actor(&frame, a).attack_target, Some(b));
}

#[test]
fn destination_and_stop_publish_the_receiving_movement_goal() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("MoveWb");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[crate::fixture_content::FixtureObject::new("MoveWA")
            .kind_of("SELECTABLE")
            .health(100.0)],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    logic
        .templates
        .get_mut("MoveWA")
        .unwrap()
        .add_kind_of(KindOf::Infantry);
    let a = logic
        .create_object("MoveWA", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("a");
    {
        let (object, mut writer) = logic.host_object_and_movement_events_mut(a).unwrap();
        object.set_destination(
            Vec3::new(50.0, 0.0, 25.0),
            0,
            &mut writer,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    let events = logic.movement_events.drain_for_host_boundary();
    assert!(!events.is_empty(), "move log should hold destination");

    let waypoint = Vec3::new(50.0, 0.0, 25.0);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].unit, a);
    assert_eq!(events[0].destination, Some(waypoint.to_array()));

    let before = publish(&mut logic);
    assert_eq!(actor(&before, a).move_destination, Some(waypoint));
    logic
        .host_object_mut(a)
        .unwrap()
        .stop_moving(&mut crate::game_logic::ObjectAudio::record(&mut Vec::new()));
    assert!(
        logic
            .host_object(a)
            .unwrap()
            .movement
            .target_position
            .is_none()
    );
    assert_eq!(actor(&publish(&mut logic), a).move_destination, None);
    assert_eq!(actor(&before, a).move_destination, Some(waypoint));
}

#[test]
fn ordinary_object_writes_feed_later_frames_and_leave_earlier_frames_frozen() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("HostStore");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[crate::fixture_content::FixtureObject::new("HostStoreU")
            .kind_of("SELECTABLE")
            .health(80.0)],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let oid = logic
        .create_object("HostStoreU", Team::USA, Vec3::new(2.0, 0.0, 3.0))
        .expect("id");
    let victim = logic
        .create_object("HostStoreU", Team::USA, Vec3::new(8.0, 0.0, 1.0))
        .expect("victim");

    let initial = publish(&mut logic);

    {
        logic
            .with_host_object_mut(oid, |o| {
                o.health.current = 60.0;
                let mut w = crate::game_logic::Weapon::default();
                w.clip_size = 6;
                w.ammo = Some(4);
                o.weapon = Some(w);
                o.movement.target_position = Some(Vec3::new(11.0, 0.0, 5.0));
                o.target = Some(victim);
            })
            .expect("shipped mutate");
        let stamped = logic
            .host_stamp_attack_substate_at_frame(oid, crate::game_logic::AttackSubState::FireWeapon)
            .expect("split-borrow store + frame");
        assert_eq!(stamped, logic.frame);
    };

    let spawn_hp = logic.host_authoritative_health(oid);
    assert_eq!(
        spawn_hp,
        Some(60.0),
        "owned body mutation is immediately visible"
    );
    let updated = publish(&mut logic);
    assert_eq!(actor(&updated, oid).health_current, 60.0);
    assert_eq!(
        actor(&updated, oid).move_destination,
        Some(Vec3::new(11.0, 0.0, 5.0))
    );
    assert_eq!(actor(&updated, oid).attack_target, Some(victim));
    assert_eq!(actor(&initial, oid).health_current, 80.0);
    assert_eq!(actor(&initial, oid).move_destination, None);
    assert_eq!(actor(&initial, oid).attack_target, None);

    if let Some(o) = logic.objects.get_mut(&oid) {
        o.health.current = 1.0;
        if let Some(w) = o.weapon.as_mut() {
            w.ammo = Some(1);
        }
        o.movement.target_position = Some(Vec3::new(99.0, 0.0, 99.0));
        o.target = Some(ObjectId(9999));
    }
    assert_eq!(
        logic.host_authoritative_health(oid),
        Some(1.0),
        "ordinary query must read the same canonical body"
    );
    assert_eq!(actor(&updated, oid).health_current, 60.0);
    assert_eq!(actor(&updated, oid).attack_target, Some(victim));

    // Split-borrow: mutate store field while reading frame.
    let frame = logic.frame;
    let obj = logic.objects.get_mut(&oid).expect("store");
    obj.set_attack_substate(crate::game_logic::AttackSubState::AimAtTarget);
    assert_eq!(frame, logic.frame);
}

#[test]
fn attack_receipts_and_published_targets_stay_with_the_receiving_world() {
    fn world() -> (GameLogic, ObjectId, ObjectId, ObjectId) {
        use crate::fixture_content::{FixtureObject, FixtureWeapon, admit_fixture_content};
        let mut logic = GameLogic::new();
        // Only an Object with an AIUpdateInterface can hold an attack state
        // (AIUpdateInterface::privateAttackObject), and AIAttackState needs a
        // WeaponSet weapon able to engage the victim; both are authored
        // before NewGame (GameEngine.cpp:428-458).
        admit_fixture_content(
            &mut logic,
            &[FixtureObject::new("OwnedAttackReceiptInfantry")
                .kind_of("INFANTRY")
                .health(100.0)
                .walker()
                .armed("OwnedAttackReceiptRifle")],
            &[FixtureWeapon::new("OwnedAttackReceiptRifle")
                .damage(10.0)
                .range(100.0)],
        );
        logic.add_player(crate::game_logic::Player::new(
            0,
            Team::USA,
            "AttackOwner",
            true,
        ));
        logic.add_player(crate::game_logic::Player::new(1, Team::GLA, "Enemy", false));
        let attacker = logic
            .create_object_for_player("OwnedAttackReceiptInfantry", 0, Vec3::ZERO)
            .unwrap();
        let first = logic
            .create_object_for_player("OwnedAttackReceiptInfantry", 1, Vec3::new(10.0, 0.0, 0.0))
            .unwrap();
        let second = logic
            .create_object_for_player("OwnedAttackReceiptInfantry", 1, Vec3::new(20.0, 0.0, 0.0))
            .unwrap();
        (logic, attacker, first, second)
    }
    fn order(logic: &mut GameLogic, attacker: ObjectId, target: ObjectId) {
        let frame = logic.get_frame();
        let (object, output) = logic.host_object_and_attack_events_mut(attacker).unwrap();
        object.attack_target(
            target,
            frame,
            output,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        assert_eq!(object.target, Some(target));
        assert_eq!(object.ai_state(), crate::game_logic::AIState::Attacking);
        assert!(object.status.attacking);
        logic.attack_events.complete_boundary();
    }
    let (mut owner, attacker, first, second) = world();
    let (mut peer, peer_attacker, peer_first, peer_second) = world();
    assert_eq!(
        (attacker, first, second),
        (peer_attacker, peer_first, peer_second)
    );
    order(&mut owner, attacker, first);
    order(&mut peer, peer_attacker, peer_second);

    let peer_frame = publish(&mut peer);
    assert_eq!(actor(&peer_frame, attacker).attack_target, Some(second));
    let peer_receipts = peer_frame
        .events
        .iter()
        .filter(|event| {
            matches!(
                event,
                crate::presentation_frame::PresentationEvent::AttackTargeted { .. }
            )
        })
        .count();
    assert_eq!(peer_receipts, 1);
    peer.reset();
    order(&mut owner, attacker, second);
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&owner).unwrap();
    let (mut peer, peer_attacker, peer_first, _) = world();
    order(&mut peer, peer_attacker, peer_first);
    builder.restore_from_snapshot(&snapshot, &mut peer).unwrap();
    assert!(peer.attack_events.take_completed().is_empty());
    assert_eq!(peer.host_object(attacker).unwrap().target, Some(second));
    owner
        .host_object_mut(attacker)
        .unwrap()
        .stop_attack(&mut crate::game_logic::ObjectAudio::record(&mut Vec::new()));
    assert!(owner.host_object(attacker).unwrap().target.is_none());
    assert!(!owner.host_object(attacker).unwrap().status.attacking);
    owner.attack_events.complete_boundary();
    let frame = publish(&mut owner);
    assert_eq!(actor(&frame, attacker).attack_target, None);
    assert_eq!(
        actor(&publish(&mut peer), attacker).attack_target,
        Some(second)
    );
    assert_eq!(actor(&peer_frame, attacker).attack_target, Some(second));
    let targeted = frame
        .events
        .iter()
        .filter_map(|event| match event {
            generals_presentation::events::PresentationEvent::AttackTargeted {
                attacker,
                target,
            } => Some((*attacker, *target)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        targeted,
        vec![(attacker, Some(first)), (attacker, Some(second))]
    );
    let repeated = publish(&mut owner);
    assert!(!repeated.events.iter().any(|event| matches!(
        event,
        generals_presentation::events::PresentationEvent::AttackTargeted { .. }
    )));
}

#[test]
fn idle_order_clears_the_target_and_enters_idle() {
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("IdleAuth");
    logic
        .admit_object_ini_sources([(
            "fixture-objects.ini".into(),
            "Object IdU\nEnd\nObject IdE\nEnd\n".into(),
        )])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for name in ["IdU", "IdE"] {
        {
            let mut t = ThingTemplate::new(name);
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Attackable);
            logic.templates.insert(name.into(), t);
        }
    }
    let oid = logic
        .create_object("IdU", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("u");
    let vid = logic
        .create_object("IdE", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .expect("e");
    if let Some(o) = logic.host_object_mut(oid) {
        o.target = Some(vid);
        o.status.attacking = true;
        o.set_ai_state(
            AIState::Attacking,
            0,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    let before = publish(&mut logic);
    assert_eq!(actor(&before, oid).attack_target, Some(vid));
    assert!(logic.private_idle_for_test(oid));

    // AIUpdate.cpp:3067 clears the state machine and enters Idle immediately.
    let idle = logic.host_objects().lookup(&oid).unwrap();
    assert!(idle.target.is_none());
    assert_eq!(idle.ai_state(), AIState::Idle);

    let frame = publish(&mut logic);
    assert_eq!(actor(&frame, oid).attack_target, None);
    assert_eq!(
        actor(&frame, oid).ai_state_ordinal,
        AIState::Idle.to_ordinal()
    );
    assert_eq!(actor(&before, oid).attack_target, Some(vid));
}

#[test]
fn private_stop_clears_published_engagement() {
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("PrivStop");
    logic
        .admit_object_ini_sources([(
            "fixture-objects.ini".into(),
            "Object PsU\nEnd\nObject PsE\nEnd\n".into(),
        )])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for name in ["PsU", "PsE"] {
        {
            let mut t = ThingTemplate::new(name);
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Attackable);
            logic.templates.insert(name.into(), t);
        }
    }
    let oid = logic
        .create_object("PsU", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("u");
    let vid = logic
        .create_object("PsE", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .expect("e");
    if let Some(o) = logic.host_object_mut(oid) {
        o.target = Some(vid);
        o.status.attacking = true;
    }
    let before = publish(&mut logic);
    assert_eq!(actor(&before, oid).attack_target, Some(vid));
    assert!(logic.private_stop(oid));
    assert!(
        logic.host_objects().lookup(&oid).unwrap().target.is_none(),
        "private_stop must clear host target immediately"
    );

    assert!(!logic.host_object(oid).unwrap().status.attacking);
    assert_eq!(actor(&publish(&mut logic), oid).attack_target, None);
    assert_eq!(actor(&before, oid).attack_target, Some(vid));
}

#[test]
fn ai_state_changes_appear_in_the_next_frame() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("AiStateCh");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object AiU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("AiU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("AiU".into(), t);
    }
    let id = logic
        .create_object("AiU", Team::USA, Vec3::new(1.0, 0.0, 1.0))
        .expect("id");
    logic.host_object_mut(id).expect("o").set_ai_state(
        AIState::GuardingObject,
        0,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    assert_eq!(
        logic.host_object(id).expect("o").ai_state(),
        AIState::GuardingObject
    );

    let before = publish(&mut logic);
    assert_eq!(actor(&before, id).ai_state_ordinal, 10);
    logic.host_object_mut(id).unwrap().set_ai_state(
        AIState::Idle,
        0,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    assert_eq!(logic.host_object(id).unwrap().ai_state(), AIState::Idle);
    assert_eq!(actor(&publish(&mut logic), id).ai_state_ordinal, 0);
    assert_eq!(actor(&before, id).ai_state_ordinal, 10);
}

#[test]
fn position_and_object_guards_are_exclusive_in_each_frame() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("GuardCh");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[crate::fixture_content::FixtureObject::new("GU")
            .kind_of("SELECTABLE")
            .health(100.0)
            .walker()],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let oid = logic
        .create_object("GU", Team::USA, Vec3::new(12.0, 0.0, 12.0))
        .expect("id");
    let tid = logic
        .create_object("GU", Team::USA, Vec3::new(14.0, 0.0, 14.0))
        .expect("tid");
    // AIUpdate.cpp stores each guard target with a type. The active target
    // selects the position or the object for this frame.
    let position = Vec3::new(3.0, 0.0, 5.0);
    logic.host_object_mut(oid).unwrap().set_guard_position(
        Some(position),
        0,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );

    let location = publish(&mut logic);
    assert_eq!(actor(&location, oid).guard_position, Some(position));
    assert_eq!(actor(&location, oid).guard_target, None);
    assert!(logic.unit_command_set_guard_radius(oid, 175.0));
    let wider = publish(&mut logic);
    assert_eq!(actor(&wider, oid).guard_position, Some(position));
    assert_eq!(actor(&wider, oid).guard_radius, 175.0);
    assert!(logic.unit_command_set_guard_radius(oid, 80.0));
    assert_eq!(actor(&publish(&mut logic), oid).guard_radius, 80.0);
    assert_eq!(actor(&wider, oid).guard_radius, 175.0);

    logic.host_object_mut(oid).unwrap().set_guard_target(
        Some(tid),
        0,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    let object = publish(&mut logic);
    assert_eq!(actor(&object, oid).guard_position, None);
    assert_eq!(actor(&object, oid).guard_target, Some(tid));
    assert_eq!(actor(&location, oid).guard_position, Some(position));

    logic.host_object_mut(oid).unwrap().set_guard_target(
        None,
        0,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    let cleared = publish(&mut logic);
    assert_eq!(actor(&cleared, oid).guard_position, None);
    assert_eq!(actor(&cleared, oid).guard_target, None);
    assert_eq!(actor(&object, oid).guard_target, Some(tid));
}

#[test]
fn target_location_publishes_and_can_be_cleared() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("TLocCh");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object TLocU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("TLocU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("TLocU".into(), t);
    }
    let oid = logic
        .create_object("TLocU", Team::USA, Vec3::new(9.0, 0.0, 9.0))
        .expect("id");

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_target_location(
            Some(Vec3::new(11.0, 0.0, 13.0)),
            0,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }

    let before = publish(&mut logic);
    let destination = Vec3::new(11.0, 0.0, 13.0);
    assert_eq!(
        logic.host_object(oid).unwrap().target_location,
        Some(destination)
    );
    assert_eq!(actor(&before, oid).target_location, Some(destination));

    logic.host_object_mut(oid).unwrap().set_target_location(
        None,
        0,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    assert_eq!(actor(&publish(&mut logic), oid).target_location, None);
    assert_eq!(actor(&before, oid).target_location, Some(destination));
}

#[test]
fn combat_intent_publishes_the_receiving_flags_and_targets() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EntityCombatIntent");
    logic
        .admit_object_ini_sources([(
            "fixture-objects.ini".into(),
            "Object CbtIntentU\nEnd\n".into(),
        )])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    insert_fixture_template(&mut logic, "CbtIntentU", 100.0);
    let id = logic
        .create_object("CbtIntentU", Team::USA, Vec3::new(4.0, 0.0, 4.0))
        .expect("id");
    let guard = logic
        .create_object("CbtIntentU", Team::USA, Vec3::new(8.0, 0.0, 4.0))
        .expect("guard");
    {
        let obj = logic.host_object_mut(id).expect("obj");
        obj.force_attack = true;
        obj.show_health_bar = false;
        obj.target_location = Some(Vec3::new(10.0, 0.0, 10.0));
        obj.set_guard_position_data(Some(Vec3::new(1.0, 0.0, 1.0)));
        obj.set_guard_target_data(Some(guard));
        obj.restore_ai_state(crate::game_logic::AIState::GuardingObject);
        obj.occupants = vec![guard];
    }

    let frame = publish(&mut logic);
    let published = actor(&frame, id);
    assert!(published.force_attack);
    assert!(!published.show_health_bar);
    assert_eq!(published.guard_target, Some(guard));
    assert_eq!(published.guard_position, None);
    assert_eq!(
        published.ai_state_ordinal,
        AIState::GuardingObject.to_ordinal()
    );
    assert_eq!(published.occupant_count, 1);
    assert_eq!(published.target_location, Some(Vec3::new(10.0, 0.0, 10.0)));
}
