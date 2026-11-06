//! C++ StealthUpdate / StealthUpgrade are selected and parameterized by the
//! authored module data (StealthUpdate.cpp:107-401, 568-814;
//! StealthUpgrade.cpp:27-42), never by the template name. Real receiving
//! admission + object creation + the production frame update.
use crate::game_logic::{GameLogic, ObjectId, Player, Team};
use glam::Vec3;

fn object(name: &str, kind: &str, modules: &str) -> String {
    format!(
        "Object {name}\nKindOf = {kind} SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\n{modules}End\n"
    )
}

fn world(objects: &[String]) -> GameLogic {
    let mut world = GameLogic::new();
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "receiving-stealth-update.ini".into(),
                objects.concat(),
            )])
            .unwrap(),
        )
        .unwrap();
    world
}

fn tick_to(world: &mut GameLogic, frame: u32) {
    while world.frame < frame {
        world.frame += 1;
        world.update_stealth_and_detection();
    }
}

fn stealthed(world: &GameLogic, id: ObjectId) -> bool {
    world.host_object(id).unwrap().status.stealthed
}

fn stealth_module(body: &str) -> String {
    format!("Behavior = StealthUpdate StealthTag\n{body}End\n")
}

#[test]
fn stealth_constructor_consumes_one_receiving_client_random_draw() {
    let mut world = world(&[
        object("Cloaked", "INFANTRY", &stealth_module("InnateStealth = Yes\n")),
        object("Unarmed", "INFANTRY", ""),
    ]);
    world.seed_fx_random(0x1234);
    let mut expected = world.session_view.client_random.clone();
    let first = expected.next_real(0.0, std::f32::consts::PI);
    let second = expected.next_real(0.0, std::f32::consts::PI);

    let a = world.create_object("Cloaked", Team::GLA, Vec3::ZERO).unwrap();
    world.create_object("Unarmed", Team::GLA, Vec3::ZERO).unwrap();
    let b = world.create_object("Cloaked", Team::GLA, Vec3::ZERO).unwrap();

    assert_eq!(world.host_object(a).unwrap().camo_opacity_pulse_phase, first);
    assert_eq!(world.host_object(b).unwrap().camo_opacity_pulse_phase, second);
    assert_eq!(world.session_view.client_random.seed_words(), expected.seed_words());
}

#[test]
fn authored_stealth_delay_gates_the_first_cloak() {
    // 1000 ms -> 30 logic frames (parseDurationUnsignedInt).
    let mut world = world(&[
        object(
            "PlainSneaker",
            "INFANTRY",
            &stealth_module("StealthDelay = 1000\nStealthForbiddenConditions = ATTACKING\nInnateStealth = Yes\n"),
        ),
        object("GLAInfantryJarmenKell", "INFANTRY", ""),
    ]);
    let sneaker = world.create_object("PlainSneaker", Team::GLA, Vec3::ZERO).unwrap();
    let named = world
        .create_object("GLAInfantryJarmenKell", Team::GLA, Vec3::X)
        .unwrap();
    assert!(world.host_object(sneaker).unwrap().innate_stealth, "InnateStealth sets CAN_STEALTH");
    assert!(!stealthed(&world, sneaker), "the constructor only arms StealthDelay");
    tick_to(&mut world, 29);
    assert!(!stealthed(&world, sneaker));
    tick_to(&mut world, 30);
    assert!(stealthed(&world, sneaker), "cloaks once m_stealthAllowedFrame passes");
    tick_to(&mut world, 120);
    assert!(!stealthed(&world, named), "a hero name without a StealthUpdate never cloaks");
    assert!(!world.host_object(named).unwrap().innate_stealth);
}

#[test]
fn firing_primary_reveals_and_rearms_the_delay_but_secondary_does_not() {
    // 2000 ms -> 60 frames.
    let mut world = world(&[object(
        "PlainSentry",
        "VEHICLE",
        &stealth_module("StealthDelay = 2000\nStealthForbiddenConditions = FIRING_PRIMARY MOVING\nInnateStealth = Yes\n"),
    )]);
    let id = world.create_object("PlainSentry", Team::USA, Vec3::ZERO).unwrap();
    tick_to(&mut world, 60);
    assert!(stealthed(&world, id));
    let mut combat = crate::game_logic::combat::CombatSystem::new();
    let mut attacks = crate::game_logic::host_attack_log::AttackEvents::default();
    let mut sounds = Vec::new();
    {
        let obj = world.host_object_mut(id).unwrap();
        obj.weapon = Some(crate::game_logic::Weapon {
            last_fire_time: -10.0,
            ..Default::default()
        });
        obj.secondary_weapon = Some(crate::game_logic::Weapon {
            last_fire_time: -10.0,
            ..Default::default()
        });
    }

    // A secondary shot while IS_FIRING_WEAPON keeps the cloak.
    {
        let obj = world.host_object_mut(id).unwrap();
        obj.set_active_weapon_slot(1);
        assert!(obj.fire_at(
            ObjectId(999), 61.0 / 30.0, 61, &mut combat, &mut attacks,
            &mut crate::game_logic::ObjectAudio::record(&mut sounds),
        ));
        assert_eq!(obj.last_fire_slot, 1);
        assert_eq!(obj.secondary_weapon.as_ref().unwrap().last_fire_frame, 61);
        obj.set_status_firing_weapon(true);
    }
    tick_to(&mut world, 61);
    assert!(stealthed(&world, id), "FIRING_PRIMARY ignores the secondary slot");

    // A primary shot this frame reveals and re-arms StealthDelay.
    {
        let obj = world.host_object_mut(id).unwrap();
        obj.set_active_weapon_slot(0);
        assert!(obj.fire_at(
            ObjectId(999), 62.0 / 30.0, 62, &mut combat, &mut attacks,
            &mut crate::game_logic::ObjectAudio::record(&mut sounds),
        ));
        assert_eq!(obj.last_fire_slot, 0);
        assert_eq!(obj.weapon.as_ref().unwrap().last_fire_frame, 62);
    }
    tick_to(&mut world, 62);
    assert!(!stealthed(&world, id));
    assert_eq!(world.host_object(id).unwrap().stealth_allowed_frame, 122);

    world.host_object_mut(id).unwrap().set_status_firing_weapon(false);
    tick_to(&mut world, 121);
    assert!(!stealthed(&world, id), "StealthDelay holds the re-cloak");
    tick_to(&mut world, 122);
    assert!(stealthed(&world, id));
}

#[test]
fn moving_above_the_authored_threshold_reveals() {
    // MoveThresholdSpeed is parseVelocityReal: 30 per second -> 1 per frame.
    let mut world = world(&[object(
        "PlainScout",
        "INFANTRY",
        &stealth_module(
            "StealthDelay = 0\nStealthForbiddenConditions = MOVING\nMoveThresholdSpeed = 30\nInnateStealth = Yes\n",
        ),
    )]);
    let id = world.create_object("PlainScout", Team::USA, Vec3::ZERO).unwrap();
    tick_to(&mut world, 1);
    assert!(stealthed(&world, id));
    world.host_object_mut(id).unwrap().movement.velocity = Vec3::new(0.5, 0.0, 0.0);
    tick_to(&mut world, 2);
    assert!(stealthed(&world, id), "below MoveThresholdSpeed stays cloaked");
    world.host_object_mut(id).unwrap().movement.velocity = Vec3::new(2.0, 0.0, 0.0);
    tick_to(&mut world, 3);
    assert!(!stealthed(&world, id), "MOVING above the threshold reveals");
}

#[test]
fn authored_required_and_forbidden_status_gate_cloak() {
    let mut world = world(&[object(
        "StatusSneaker",
        "INFANTRY",
        &stealth_module(
            "StealthDelay = 0\nInnateStealth = Yes\nRequiredStatus = BOOBY_TRAPPED\nForbiddenStatus = DETECTED\n",
        ),
    )]);
    let id = world.create_object("StatusSneaker", Team::USA, Vec3::ZERO).unwrap();
    tick_to(&mut world, 1);
    assert!(!stealthed(&world, id));
    world.host_object_mut(id).unwrap().status.booby_trapped = true;
    tick_to(&mut world, 2);
    assert!(stealthed(&world, id));
    world.host_object_mut(id).unwrap().set_status_detected(true);
    tick_to(&mut world, 3);
    assert!(!stealthed(&world, id));
}

#[test]
fn detection_reveal_wakes_idle_enemies_only_for_authored_order_idle() {
    let mut world = world(&[
        object(
            "LoudSneaker",
            "INFANTRY",
            &stealth_module(
                "StealthDelay = 1000\nInnateStealth = Yes\nOrderIdleEnemiesToAttackMeUponReveal = Yes\n",
            ),
        ),
        object("QuietSneaker", "INFANTRY", &stealth_module("StealthDelay = 1000\nInnateStealth = Yes\n")),
        object("PlainGuard", "INFANTRY", ""),
    ]);
    let loud = world.create_object("LoudSneaker", Team::GLA, Vec3::ZERO).unwrap();
    let quiet = world.create_object("QuietSneaker", Team::GLA, Vec3::new(0.0, 0.0, 400.0)).unwrap();
    let guard = world.create_object("PlainGuard", Team::USA, Vec3::new(10.0, 0.0, 0.0)).unwrap();
    let quiet_guard = world
        .create_object("PlainGuard", Team::USA, Vec3::new(10.0, 0.0, 400.0))
        .unwrap();
    for id in [guard, quiet_guard] {
        let obj = world.host_object_mut(id).unwrap();
        obj.vision_range = 100.0;
        obj.next_mood_check_time = 999;
    }
    tick_to(&mut world, 30);
    assert!(stealthed(&world, loud) && stealthed(&world, quiet));

    // markAsDetected(): numFrames 0 -> now + StealthDelay.
    world.mark_stealth_detected(loud, 0);
    world.mark_stealth_detected(quiet, 0);
    let loud_obj = world.host_object(loud).unwrap();
    assert!(!loud_obj.status.detected, "marking only updates the timer");
    assert_eq!(loud_obj.detection_expires_frame, 60);
    assert_eq!(world.host_object(guard).unwrap().next_mood_check_time, 30);
    assert_eq!(
        world.host_object(quiet_guard).unwrap().next_mood_check_time,
        999,
        "no OrderIdleEnemiesToAttackMeUponReveal, no wake"
    );
    tick_to(&mut world, 31);
    assert!(world.host_object(loud).unwrap().status.detected);
    tick_to(&mut world, 60);
    assert!(!world.host_object(loud).unwrap().status.detected, "detection expires");
}

#[test]
fn stealth_upgrade_grants_can_stealth_to_existing_and_late_built_units() {
    let modules = format!(
        "{}Behavior = StealthUpgrade CamoTag\nTriggeredBy = Upgrade_TestCamo\nEnd\n",
        stealth_module("StealthDelay = 0\nStealthForbiddenConditions = ATTACKING\nInnateStealth = No\n"),
    );
    let mut world = world(&[object("PlainRebel", "INFANTRY", &modules)]);
    world.players.insert(0, Player::new(0, Team::GLA, "Owner", true));
    let early = world.create_object_for_player("PlainRebel", 0, Vec3::ZERO).unwrap();
    tick_to(&mut world, 5);
    assert!(!world.host_object(early).unwrap().innate_stealth);
    assert!(!stealthed(&world, early), "InnateStealth No waits for the upgrade");

    world
        .players
        .get_mut(&0)
        .unwrap()
        .add_completed_upgrade("Upgrade_TestCamo");
    world
        .apply_player_research_to_object_with_interpreter_checked(
            early,
            "Upgrade_TestCamo",
            None,
            &mut Default::default(),
        )
        .unwrap();
    assert!(world.host_object(early).unwrap().innate_stealth, "StealthUpgrade sets CAN_STEALTH");
    tick_to(&mut world, 6);
    assert!(stealthed(&world, early));

    // Object::initObject -> updateUpgradeModules replays the completed upgrade.
    let late = world.create_object_for_player("PlainRebel", 0, Vec3::X).unwrap();
    assert!(world.host_object(late).unwrap().innate_stealth);
    tick_to(&mut world, 7);
    assert!(stealthed(&world, late));
}

#[test]
fn detection_status_waits_for_awake_update_and_cloak_delay() {
    let mut world = world(&[object(
        "DelayedDetectorTarget",
        "INFANTRY",
        &stealth_module("StealthDelay = 1000\nInnateStealth = Yes\n"),
    )]);
    let id = world.create_object("DelayedDetectorTarget", Team::GLA, Vec3::ZERO).unwrap();
    world.mark_stealth_detected(id, 60);
    tick_to(&mut world, 1);
    assert!(!world.host_object(id).unwrap().status.detected, "cloak delay returns before detection");
    world.host_object_mut(id).unwrap().set_status_disabled_emp(true);
    tick_to(&mut world, 30);
    assert!(!world.host_object(id).unwrap().status.detected, "disabled update does not apply detection");
    world.host_object_mut(id).unwrap().set_status_disabled_emp(false);
    tick_to(&mut world, 31);
    assert!(world.host_object(id).unwrap().status.detected);
    world.host_object_mut(id).unwrap().set_status_disabled_emp(true);
    tick_to(&mut world, 60);
    assert!(world.host_object(id).unwrap().status.detected, "disabled update does not expire detection");
    world.host_object_mut(id).unwrap().set_status_disabled_emp(false);
    tick_to(&mut world, 61);
    assert!(!world.host_object(id).unwrap().status.detected);
}

#[test]
fn stealth_upgrade_grants_only_current_weapon_spawn_roster() {
    let modules = format!(
        "{}Behavior = SpawnBehavior Spawn\nOneShot = Yes\nSpawnNumber = 1\nSpawnTemplateName = UpgradeSlave\nEnd\nBehavior = StealthUpgrade Upgrade\nTriggeredBy = Upgrade_TestCamo\nEnd\n",
        stealth_module("StealthDelay = 0\nInnateStealth = No\n")
    );
    let mut world = world(&[
        object("UpgradeNest", "STRUCTURE SPAWNS_ARE_THE_WEAPONS", &modules),
        object("OrdinaryNest", "STRUCTURE", &modules),
        object("UpgradeSlave", "INFANTRY", &stealth_module("StealthDelay = 0\nInnateStealth = No\n")),
    ]);
    world.players.insert(0, Player::new(0, Team::GLA, "Owner", true));
    let nest = world.create_object_for_player("UpgradeNest", 0, Vec3::ZERO).unwrap();
    let ordinary = world.create_object_for_player("OrdinaryNest", 0, Vec3::X).unwrap();
    let child = world.host_object(nest).unwrap().spawn_behavior_children()[0].0;
    let ordinary_child = world.host_object(ordinary).unwrap().spawn_behavior_children()[0].0;
    assert!(!world.host_object(child).unwrap().innate_stealth);

    world.players.get_mut(&0).unwrap().add_completed_upgrade("Upgrade_TestCamo");
    for id in [nest, ordinary] {
        world.apply_player_research_to_object_with_interpreter_checked(
            id, "Upgrade_TestCamo", None, &mut Default::default(),
        ).unwrap();
    }
    assert!(world.host_object(nest).unwrap().innate_stealth);
    assert!(world.host_object(child).unwrap().innate_stealth);
    assert!(!world.host_object(child).unwrap().status.stealthed, "the upgrade sets CAN_STEALTH only");
    assert!(!world.host_object(ordinary_child).unwrap().innate_stealth);

    // SpawnBehavior::createSpawn does not copy the parent's CAN_STEALTH:
    // the parent's already-completed upgrade ran before this child existed.
    let late = world.create_object_for_player("UpgradeNest", 0, Vec3::Z).unwrap();
    let late_child = world.host_object(late).unwrap().spawn_behavior_children()[0].0;
    assert!(world.host_object(late).unwrap().innate_stealth);
    assert!(!world.host_object(late_child).unwrap().innate_stealth);
}
