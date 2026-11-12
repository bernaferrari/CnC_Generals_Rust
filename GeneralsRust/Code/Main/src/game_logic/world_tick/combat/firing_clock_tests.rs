//! Receiving shot clocks and one canonical FiringTracker across all lanes.
use super::*;
use crate::game_logic::object::WeaponFireStatus;


fn authored_clock_world(
    min_delay: i32, max_delay: i32, clip_size: i32, shared: bool,
) -> (GameLogic, ObjectId, ObjectId) {
    authored_clock_world_with_setup(min_delay, max_delay, clip_size, shared, |_| {})
}

fn authored_clock_world_with_setup(
    min_delay: i32, max_delay: i32, clip_size: i32, shared: bool,
    setup: impl FnOnce(&mut GameLogic),
) -> (GameLogic, ObjectId, ObjectId) {
    let mut game = GameLogic::new();
    let mut primary = gamelogic::weapon::WeaponTemplate::new("DeadlinePrimary".into());
    primary.primary_damage = 1.0;
    primary.attack_range = 100.0;
    primary.min_delay_between_shots = min_delay;
    primary.max_delay_between_shots = max_delay;
    primary.clip_size = clip_size;
    primary.clip_reload_time = 60;
    primary.continuous_fire_coast_frames = 7;
    primary.continuous_fire_one_shots_needed = 2;
    primary.continuous_fire_two_shots_needed = 4;
    primary.projectile_name = "NONE".into();
    let mut secondary = primary.clone();
    secondary.name = "DeadlineSecondary".into();
    secondary.clip_reload_time = 180;
    game.admit_receiving_weapon_snapshot(vec![
        ("DeadlinePrimary".into(), primary), ("DeadlineSecondary".into(), secondary),
    ]).unwrap();
    game.admit_receiving_module_content(
        crate::assets::ReceivingObjectContent::from_sources([(
            "integer-clock.ini".into(),
            format!("Object DeadlineActor\nKindOf = INFANTRY CAN_ATTACK\n\
                WeaponSet\nConditions = None\nWeapon = PRIMARY DeadlinePrimary\n\
                Weapon = SECONDARY DeadlineSecondary\nShareWeaponReloadTime = {}\nEnd\n\
                Body = ActiveBody BodyTag\nMaxHealth = 1000\nInitialHealth = 1000\nEnd\n\
                Behavior = AIUpdateInterface AI\nEnd\nEnd\n\
                Object ClockGarrison\nKindOf = STRUCTURE IMMOBILE\n\
                Body = ActiveBody Body\nMaxHealth = 1000\nInitialHealth = 1000\nEnd\n\
                Behavior = GarrisonContain Contain\nContainMax = 5\nAllowInsideKindOf = INFANTRY\nEnd\nEnd\n\
                Object ClockHelix\nKindOf = VEHICLE AIRCRAFT\n\
                Body = ActiveBody Body\nMaxHealth = 1000\nInitialHealth = 1000\nEnd\n\
                Behavior = HelixContain Contain\nSlots = 5\nAllowInsideKindOf = INFANTRY\nEnd\nEnd\n",
                if shared { "Yes" } else { "No" }),
        )]).unwrap(),
    ).unwrap();
    setup(&mut game);
    game.add_player(Player::new(0, Team::China, "ClockOwner", true));
    game.add_player(Player::new(1, Team::USA, "ClockTarget", false));
    let shooter = game.create_object_for_player("DeadlineActor", 0, Vec3::ZERO).unwrap();
    let target = game.create_object_for_player("DeadlineActor", 1, Vec3::new(20.0, 0.0, 0.0)).unwrap();
    (game, shooter, target)
}

#[test]
fn authored_random_restart_consumes_one_receiving_draw_and_expiry_consumes_none() {
    let (mut game, shooter, target) = authored_clock_world(30, 90, 0, false);
    game.logic_random.seed_random(412);
    game.frame = 15;
    assert!(game.fire_owned_passive_weapon(shooter, target, 0, Vec3::ZERO));
    let actor = game.host_object(shooter).unwrap();
    let deadline = actor.weapon.as_ref().unwrap().next_fire_frame;
    assert_eq!(actor.continuous_fire_coast_until_frame, deadline + 7);
    let expected = game_engine::common::random_value::SessionRandom::from_state(game.logic_random.snapshot());
    let expected_delay = (expected.next_int(30, 90) as f32 / 1.5).floor() as u32;
    game.objects.get_mut(&shooter).unwrap().on_owned_weapon_bonus_change(
        35, &[1.5; 3], &game.logic_random,
    );
    let deadline = 35 + expected_delay;
    assert_eq!(game.objects[&shooter].weapon.as_ref().unwrap().next_fire_frame, deadline);
    assert_eq!(game.logic_random.seed_words(), expected.seed_words());
    // A READY slot must not restart or draw, even if its ROF changes.
    game.objects.get_mut(&shooter).unwrap().on_owned_weapon_bonus_change(
        deadline, &[2.0; 3], &game.logic_random,
    );
    assert_eq!(game.objects[&shooter].weapon.as_ref().unwrap().next_fire_frame, deadline);
    assert_eq!(game.logic_random.seed_words(), expected.seed_words());
    game.frame = deadline - 1;
    assert!(!game.fire_owned_passive_weapon(shooter, target, 0, Vec3::ZERO));
    game.frame = deadline;
    assert!(game.fire_owned_passive_weapon(shooter, target, 0, Vec3::ZERO));
}

#[test]
fn authored_shared_reload_copies_source_deadline_not_sibling_interval() {
    let (mut game, shooter, target) = authored_clock_world(30, 90, 1, true);
    game.frame = 15;
    assert!(game.fire_owned_passive_weapon(shooter, target, 0, Vec3::ZERO));
    let actor = game.host_object(shooter).unwrap();
    assert_eq!(actor.weapon.as_ref().unwrap().next_fire_frame, 75);
    assert_eq!(actor.secondary_weapon.as_ref().unwrap().next_fire_frame, 75);
    assert_eq!(actor.continuous_fire_coast_until_frame, 82);
    assert_eq!(actor.weapon.as_ref().unwrap().ammo, Some(1));
    let before = game.logic_random.seed_words();
    game.objects.get_mut(&shooter).unwrap().on_owned_weapon_bonus_change(
        35, &[1.5; 3], &game.logic_random,
    );
    // WeaponSet visits PRIMARY then SECONDARY. Both see RELOADING_CLIP;
    // secondary's 180/1.5 restart is the last shared deadline, copied exactly.
    for slot in 0..2 {
        let weapon = game.objects[&shooter].weapon_slot(slot).unwrap();
        assert_eq!(weapon.next_fire_frame, 155);
        assert!(weapon.reloading_clip);
    }
    assert_eq!(game.logic_random.seed_words(), before);
    game.frame = 154;
    assert!(!game.fire_owned_passive_weapon(shooter, target, 1, Vec3::ZERO));
    game.frame = 155;
    assert!(game.fire_owned_passive_weapon(shooter, target, 1, Vec3::ZERO));
}

#[test]
fn authored_zero_delay_allows_same_frame_and_tiny_positive_rof_is_not_clamped() {
    let (mut zero, shooter, target) = authored_clock_world(0, 0, 0, false);
    zero.frame = 15;
    assert!(zero.fire_owned_passive_weapon(shooter, target, 0, Vec3::ZERO));
    assert_eq!(zero.objects[&shooter].weapon.as_ref().unwrap().next_fire_frame, 15);
    assert!(zero.fire_owned_passive_weapon(shooter, target, 0, Vec3::ZERO));
    let (mut slow, shooter, target) = authored_clock_world(30, 30, 0, false);
    slow.frame = 15;
    assert!(slow.fire_owned_passive_weapon(shooter, target, 0, Vec3::ZERO));
    slow.objects.get_mut(&shooter).unwrap().on_owned_weapon_bonus_change(
        35, &[0.005; 3], &slow.logic_random,
    );
    assert_eq!(slow.objects[&shooter].weapon.as_ref().unwrap().next_fire_frame, 6035);
}

#[test]
fn authored_random_deadline_survives_encoded_world_and_continues_at_exact_frame() {
    let (mut source, shooter, target) = authored_clock_world(30, 90, 0, false);
    source.frame = 15;
    assert!(source.fire_owned_passive_weapon(shooter, target, 0, Vec3::ZERO));
    source.objects.get_mut(&shooter).unwrap().on_owned_weapon_bonus_change(
        35, &[1.5; 3], &source.logic_random,
    );
    let deadline = source.objects[&shooter].weapon.as_ref().unwrap().next_fire_frame;
    let builder = crate::save_load::SnapshotBuilder::new();
    let bytes = bincode_legacy::serialize(&builder.create_world_snapshot(&source).unwrap()).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let (mut restored, _, _) = authored_clock_world(30, 90, 0, false);
    builder.restore_from_snapshot(&decoded, &mut restored).unwrap();
    for game in [&mut source, &mut restored] {
        game.frame = deadline - 1;
        assert!(!game.fire_owned_passive_weapon(shooter, target, 0, Vec3::ZERO));
        game.frame = deadline;
        assert!(game.fire_owned_passive_weapon(shooter, target, 0, Vec3::ZERO));
    }
    assert_eq!(source.objects[&shooter].weapon.as_ref().unwrap().next_fire_frame,
        restored.objects[&shooter].weapon.as_ref().unwrap().next_fire_frame);
    assert_eq!(source.logic_random.seed_words(), restored.logic_random.seed_words());
}
fn world(name: &str) -> (GameLogic, ObjectId, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::USA, "FiringOwner", true));
    world.add_player(Player::new(2, Team::GLA, "FiringTarget", false));
    let mut source = ThingTemplate::new(name);
    source.set_health(1000.0).add_kind_of(KindOf::Vehicle);
    // The gun is authored on the template (WeaponSet -> TheWeaponStore), so a
    // loading world that receives these templates rebuilds the same weapon.
    let gun = Weapon {
        damage: 1.0,
        range: 1000.0,
        clip_size: 6,
        ..Default::default()
    };
    crate::fixture_content::arm_template(
        &mut source,
        [
            Some(
                crate::fixture_content::FixtureWeapon::from_stats("FiringClockGun", &gun)
                    // WeaponTemplate ContinuousFireOne/Two/Coast: the tracker
                    // reads them from the fired weapon (FiringTracker.cpp).
                    .field("ContinuousFireOne = 2")
                    .field("ContinuousFireTwo = 6")
                    .field("ContinuousFireCoast = 1000"),
            ),
            None,
            None,
        ],
    );
    let mut target = ThingTemplate::new("ClockTarget");
    target.set_health(1000.0).add_kind_of(KindOf::Attackable);
    world.templates.insert(name.into(), source);
    world.templates.insert("ClockTarget".into(), target);
    let source = world.create_object_for_player(name, 1, Vec3::ZERO).unwrap();
    let first = world
        .create_object_for_player("ClockTarget", 2, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    let second = world
        .create_object_for_player("ClockTarget", 2, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    let actor = world.host_object_mut(source).unwrap();
    assert!(actor.weapon.is_some(), "authored WeaponSet instantiates PRIMARY");
    actor.consecutive_shots_at_target = 4;
    actor.consecutive_shot_target = Some(first);
    actor.continuous_fire_level = 1;
    actor.continuous_fire_one_shots = 2;
    actor.continuous_fire_two_shots = 6;
    actor.continuous_fire_coast_frames = 30;
    actor.continuous_fire_coast_until_frame = 100;
    actor.pre_attack_ready_at = 5.0;
    (world, source, first, second)
}

fn commit(world: &mut GameLogic, source: ObjectId, target: ObjectId, frame: u32) {
    world.frame = frame;
    world.host_object_mut(source).unwrap().target = Some(target);
    world.commit_combat_shot(
        CombatAttackInput {
            attacker_id: source,
            current_time: frame as f32 / 30.0,
            attacker_team: Team::USA,
            target_id: Some(target),
            target_location: None,
            overcharge: false,
        },
        0,
        None,
    );
}

#[test]
fn actual_accepted_shots_use_receiving_clock_and_strict_pre_shot_coast_once() {
    // FiringTracker.cpp:83-99 preserves retarget counts strictly before coast;
    // at coast a new target resets to one, then <One/<Two invokes coolDown.
    for (level, count) in [(1, 3), (2, 5)] {
        for (frame, retained) in [(99, true), (100, false)] {
            let (mut game, shooter, target) = authored_clock_world(30, 30, 0, false);
            let actor = game.objects.get_mut(&shooter).unwrap();
            actor.continuous_fire_level = level;
            actor.consecutive_shots_at_target = count;
            actor.consecutive_shot_target = None;
            actor.continuous_fire_coast_until_frame = 100;
            game.frame = frame;
            assert!(game.fire_owned_passive_weapon(shooter, target, 0, Vec3::ZERO));
            let actor = &game.objects[&shooter];
            assert_eq!(actor.consecutive_shots_at_target, if retained { count + 1 } else { 0 });
            assert_eq!(actor.consecutive_shot_target, if retained { Some(target) } else { None });
            assert_eq!(actor.continuous_fire_level, if retained { level } else { 0 });
            game.objects.get_mut(&shooter).unwrap().status.weapons_jammed = true;
            let before = game.objects[&shooter].consecutive_shots_at_target;
            assert!(!game.fire_owned_passive_weapon(shooter, target, 0, Vec3::ZERO));
            assert_eq!(game.objects[&shooter].consecutive_shots_at_target, before);
        }
    }
    // A same-target shot exactly at One remains Mean: the comparison is <One.
    let (mut game, shooter, target) = authored_clock_world(30, 30, 0, false);
    let actor = game.objects.get_mut(&shooter).unwrap();
    actor.continuous_fire_level = 1;
    actor.consecutive_shots_at_target = 1;
    actor.consecutive_shot_target = Some(target);
    game.frame = 100;
    assert!(game.fire_owned_passive_weapon(shooter, target, 0, Vec3::ZERO));
    assert_eq!(game.objects[&shooter].consecutive_shots_at_target, 2);
    assert_eq!(game.objects[&shooter].continuous_fire_level, 1);
}

#[test]
fn normal_world_codec_retains_specialized_next_shot_and_omits_idle_reload_timer() {
    let (mut original, source, _, target) = world("ChinaVehicleGattlingTank");
    commit(&mut original, source, target, 99);
    original
        .host_object_mut(source)
        .unwrap()
        .frame_to_force_reload = 101;
    original
        .host_object_mut(source)
        .unwrap()
        .weapon_slot_mut(0)
        .unwrap()
        .ammo = Some(2);
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&original).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let snapshot = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let mut loaded = GameLogic::new();
    loaded.templates = original.templates.clone();
    loaded.add_player(Player::new(1, Team::USA, "FiringOwner", true));
    loaded.add_player(Player::new(2, Team::GLA, "FiringTarget", false));
    builder
        .restore_from_snapshot(&snapshot, &mut loaded)
        .unwrap();
    assert_eq!(
        loaded
            .host_object(source)
            .unwrap()
            .consecutive_shots_at_target,
        5
    );
    assert_eq!(
        loaded.host_object(source).unwrap().consecutive_shot_target,
        Some(target)
    );
    assert_eq!(loaded.host_object(source).unwrap().continuous_fire_level, 1);
    assert_eq!(
        loaded
            .host_object(source)
            .unwrap()
            .continuous_fire_coast_until_frame,
        original
            .host_object(source)
            .unwrap()
            .continuous_fire_coast_until_frame
    );
    // CPP constructor zero plus xfer omission prevents idle reloading after
    // load. Exercise its actual update on a separate normally restored owner.
    let mut reload_owner = GameLogic::new();
    reload_owner.templates = original.templates.clone();
    reload_owner.add_player(Player::new(1, Team::USA, "FiringOwner", true));
    reload_owner.add_player(Player::new(2, Team::GLA, "FiringTarget", false));
    builder
        .restore_from_snapshot(&snapshot, &mut reload_owner)
        .unwrap();
    reload_owner.frame = 150;
    reload_owner
        .host_object_mut(source)
        .unwrap()
        .tick_force_reload_when_idle(150);
    assert_eq!(
        reload_owner
            .host_object(source)
            .unwrap()
            .weapon
            .as_ref()
            .unwrap()
            .ammo,
        Some(2)
    );
    for owner in [&mut original, &mut loaded] {
        commit(owner, source, target, 100);
        assert_eq!(
            owner
                .host_object(source)
                .unwrap()
                .consecutive_shots_at_target,
            6
        );
        assert_eq!(
            owner.host_object(source).unwrap().consecutive_shot_target,
            Some(target)
        );
        assert_eq!(owner.host_object(source).unwrap().continuous_fire_level, 1);
    }
}

#[test]
fn generic_tracker_arms_base_and_preserves_two_stage_strict_cooldown() {
    // Exercise real authored integer commits, not a nominal f32 projection
    // made before a residual shot changes its spin-up bonus.
    for delay in [0, 4] {
        for level in [0, 1, 2] {
            let (mut game, shooter, target) = authored_clock_world(delay, delay, 0, false);
            let actor = game.objects.get_mut(&shooter).unwrap();
            actor.continuous_fire_level = level;
            actor.consecutive_shots_at_target = match level { 0 => 0, 1 => 3, _ => 5 };
            actor.consecutive_shot_target = Some(target);
            game.frame = 100;
            assert!(game.fire_owned_passive_weapon(shooter, target, 0, Vec3::ZERO));
            let deadline = 100 + delay as u32 + 7;
            assert_eq!(game.objects[&shooter].continuous_fire_coast_until_frame, deadline);
            game.frame = deadline;
            game.tick_owned_firing_tracker_coast(shooter);
            assert_eq!(game.objects[&shooter].continuous_fire_level, level);
            game.frame += 1;
            game.tick_owned_firing_tracker_coast(shooter);
            assert_eq!(game.objects[&shooter].continuous_fire_level, 0);
            assert_eq!(game.objects[&shooter].consecutive_shots_at_target, 0);
            assert_eq!(game.objects[&shooter].consecutive_shot_target, None);
            if level != 0 {
                assert_eq!(game.objects[&shooter].continuous_fire_coast_until_frame, deadline + 31);
                game.frame = deadline + 31;
                game.tick_owned_firing_tracker_coast(shooter);
                assert_eq!(game.objects[&shooter].continuous_fire_coast_until_frame, deadline + 31);
                game.frame += 1;
                game.tick_owned_firing_tracker_coast(shooter);
            }
            assert_eq!(game.objects[&shooter].continuous_fire_coast_until_frame, 0);
        }
    }
}

#[test]
fn battle_plan_condition_edges_restart_busy_weapon_even_without_rof_change() {
    use crate::game_logic::host_strategy_center::HostBattlePlan;
    let (mut world, shooter, target) = authored_clock_world(30, 90, 0, false);
    world.frame = 15;
    assert!(world.fire_owned_passive_weapon(shooter, target, 0, Vec3::ZERO));
    world.objects.get_mut(&shooter).unwrap().weapon_bonus_battle_plan_hold_the_line = true;
    world.frame = 35;
    let expected = game_engine::common::random_value::SessionRandom::from_state(world.logic_random.snapshot());
    // Player.cpp sets BOMBARDMENT first, then clears HOLDTHELINE.
    let _ = expected.next_int(30, 90);
    let delay = expected.next_int(30, 90) as u32;
    world.set_owned_battle_plan_bonus(shooter, Some(HostBattlePlan::Bombardment));
    assert_eq!(world.objects[&shooter].weapon.as_ref().unwrap().next_fire_frame, 35 + delay);
    assert_eq!(world.logic_random.seed_words(), expected.seed_words());
    world.set_owned_battle_plan_bonus(shooter, Some(HostBattlePlan::Bombardment));
    assert_eq!(world.logic_random.seed_words(), expected.seed_words(),
        "an unchanged condition must not replay the callback");
}

#[test]
fn solo_normal_toggle_restarts_busy_weapon_without_health_or_rof_change() {
    let (mut world, shooter, target) = authored_clock_world(30, 90, 0, false);
    world.game_mode = GameMode::SinglePlayer;
    world.admit_session_difficulty(1);
    world.set_owned_object_difficulty_bonus(false);
    world.frame = 15;
    assert!(world.fire_owned_passive_weapon(shooter, target, 0, Vec3::ZERO));
    world.frame = 35;
    let health = world.objects[&shooter].health.maximum;
    let expected = game_engine::common::random_value::SessionRandom::from_state(world.logic_random.snapshot());
    let delay = expected.next_int(30, 90) as u32;
    world.set_owned_object_difficulty_bonus(true);
    assert_eq!(world.objects[&shooter].health.maximum, health);
    assert_eq!(world.objects[&shooter].weapon.as_ref().unwrap().next_fire_frame, 35 + delay);
    assert_eq!(world.logic_random.seed_words(), expected.seed_words());
    world.set_owned_object_difficulty_bonus(true);
    assert_eq!(world.logic_random.seed_words(), expected.seed_words());
    let delay = expected.next_int(30, 90) as u32;
    world.set_owned_object_difficulty_bonus(false);
    assert_eq!(world.objects[&shooter].weapon.as_ref().unwrap().next_fire_frame, 35 + delay);
    assert_eq!(world.logic_random.seed_words(), expected.seed_words());
}

#[test]
fn authored_fast_transition_emits_fast_then_mean_clear_and_coast_observes_pre_transition_deadline() {
    let (mut game, shooter, target) = authored_clock_world_with_setup(30, 90, 0, false, |game| {
        game.admit_combat_bonus_ini_sources([
            "GameData\nWeaponBonus = CONTINUOUS_FIRE_MEAN RATE_OF_FIRE 150%\nWeaponBonus = CONTINUOUS_FIRE_FAST RATE_OF_FIRE 200%\nEnd\n",
        ]).unwrap();
    });
    game.frame = 15;
    for _ in 0..4 {
        assert!(game.fire_owned_passive_weapon(shooter, target, 0, Vec3::ZERO));
        game.frame = game.objects[&shooter].weapon.as_ref().unwrap().next_fire_frame;
    }
    assert_eq!(game.objects[&shooter].continuous_fire_level, 1);
    let expected = game_engine::common::random_value::SessionRandom::from_state(game.logic_random.snapshot());
    let shot_delay = (expected.next_int(30, 90) as f32 / 1.5).floor() as u32;
    // Setting FAST while MEAN remains produces additive 250%, then 200%.
    let _ = expected.next_int(30, 90);
    let fast_delay = (expected.next_int(30, 90) as f32 / 2.0).floor() as u32;
    assert!(game.fire_owned_passive_weapon(shooter, target, 0, Vec3::ZERO));
    let actor = &game.objects[&shooter];
    assert_eq!(actor.continuous_fire_level, 2);
    assert_eq!(actor.weapon.as_ref().unwrap().next_fire_frame, game.frame + fast_delay);
    assert_eq!(actor.continuous_fire_coast_until_frame, game.frame + shot_delay + 7);
    assert_eq!(game.logic_random.seed_words(), expected.seed_words());
}

#[test]
fn authored_target_faerie_is_a_post_shot_condition_and_coast_uses_its_restart() {
    let (mut game, shooter, target) = authored_clock_world_with_setup(30, 90, 0, false, |game| {
        game.admit_combat_bonus_ini_sources([
            "GameData\nWeaponBonus = TARGET_FAERIE_FIRE RATE_OF_FIRE 150%\nEnd\n",
        ]).unwrap();
    });
    game.objects.get_mut(&target).unwrap().apply_faerie_fire(1000);
    game.frame = 15;
    let expected = game_engine::common::random_value::SessionRandom::from_state(game.logic_random.snapshot());
    let _first_unbonused_shot = expected.next_int(30, 90);
    let bonused_delay = (expected.next_int(30, 90) as f32 / 1.5).floor() as u32;
    assert!(game.fire_owned_passive_weapon(shooter, target, 0, Vec3::ZERO));
    let actor = &game.objects[&shooter];
    assert!(actor.weapon_bonus_target_faerie_fire);
    assert_eq!(actor.weapon.as_ref().unwrap().next_fire_frame, 15 + bonused_delay);
    assert_eq!(actor.continuous_fire_coast_until_frame, 22 + bonused_delay);
    assert_eq!(game.logic_random.seed_words(), expected.seed_words());
    game.frame = 15 + bonused_delay;
    game.objects.get_mut(&target).unwrap().clear_faerie_fire();
    let _last_bonused_shot = expected.next_int(30, 90);
    let delay = expected.next_int(30, 90) as u32;
    assert!(game.fire_owned_passive_weapon(shooter, target, 0, Vec3::ZERO));
    assert!(!game.objects[&shooter].weapon_bonus_target_faerie_fire);
    assert_eq!(game.objects[&shooter].weapon.as_ref().unwrap().next_fire_frame, game.frame + delay);
    assert_eq!(game.logic_random.seed_words(), expected.seed_words());
}

#[test]
fn authored_veterancy_callbacks_clear_old_rank_before_setting_new_rank() {
    use crate::game_logic::VeterancyLevel;
    let (mut game, shooter, target) = authored_clock_world_with_setup(30, 90, 0, false, |game| {
        game.admit_combat_bonus_ini_sources([
            "GameData\nWeaponBonus = VETERAN RATE_OF_FIRE 150%\nWeaponBonus = ELITE RATE_OF_FIRE 200%\nEnd\n",
        ]).unwrap();
    });
    game.set_exact_veterancy_level_with_upgrade_callback_quiet(
        shooter, VeterancyLevel::Veteran, &mut Default::default(),
    ).unwrap();
    game.frame = 15;
    assert!(game.fire_owned_passive_weapon(shooter, target, 0, Vec3::ZERO));
    game.frame = 16;
    let expected = game_engine::common::random_value::SessionRandom::from_state(game.logic_random.snapshot());
    let _unranked_restart = expected.next_int(30, 90);
    let elite_delay = (expected.next_int(30, 90) as f32 / 2.0).floor() as u32;
    game.set_exact_veterancy_level_with_upgrade_callback_quiet(
        shooter, VeterancyLevel::Elite, &mut Default::default(),
    ).unwrap();
    assert_eq!(game.objects[&shooter].weapon.as_ref().unwrap().next_fire_frame, 16 + elite_delay);
    assert_eq!(game.logic_random.seed_words(), expected.seed_words());
}

fn clock_container(world: &mut GameLogic, helix: bool) -> ObjectId {
    let name = if helix { "ClockHelix" } else { "ClockGarrison" };
    world.create_object_for_player(name, 0, Vec3::ZERO).unwrap()
}

#[test]
fn garrison_callbacks_restart_before_containment_reference_changes() {
    let (mut world, shooter, target) = authored_clock_world(30, 90, 0, false);
    let container = clock_container(&mut world, false);
    world.frame = 15;
    assert!(world.fire_owned_passive_weapon(shooter, target, 0, Vec3::ZERO));
    assert!(world.objects.get_mut(&container).unwrap().add_occupant(shooter));
    world.objects.get_mut(&shooter).unwrap().contained_by = Some(container);
    assert!(!world.objects[&shooter].weapon_bonus_garrisoned,
        "the reference alone is not GarrisonContain::onContaining");
    world.frame = 35;
    let expected = game_engine::common::random_value::SessionRandom::from_state(world.logic_random.snapshot());
    let delay = expected.next_int(30, 90) as u32;
    world.apply_garrison_contain_on_enter(container, shooter);
    assert!(world.objects[&shooter].weapon_bonus_garrisoned);
    assert_eq!(world.objects[&shooter].weapon.as_ref().unwrap().next_fire_frame, 35 + delay);
    assert_eq!(world.logic_random.seed_words(), expected.seed_words());
    let delay = expected.next_int(30, 90) as u32;
    assert!(world.remove_owned_occupant_with_bonus_callback(container, shooter, &mut Default::default()));
    assert!(!world.objects[&shooter].weapon_bonus_garrisoned);
    assert_eq!(world.objects[&shooter].contained_by, Some(container),
        "onRemoving clears the condition before Object::onRemovedFrom");
    assert_eq!(world.objects[&shooter].weapon.as_ref().unwrap().next_fire_frame, 35 + delay);
    assert_eq!(world.logic_random.seed_words(), expected.seed_words());
    assert!(!world.remove_owned_occupant_with_bonus_callback(container, shooter, &mut Default::default()));
    assert_eq!(world.logic_random.seed_words(), expected.seed_words());
}

#[test]
fn helix_death_scatter_draws_precede_bonus_restart_and_keep_reference_installed() {
    let (mut world, shooter, target) = authored_clock_world(30, 90, 0, false);
    let container = clock_container(&mut world, true);
    world.frame = 15;
    assert!(world.fire_owned_passive_weapon(shooter, target, 0, Vec3::ZERO));
    assert!(world.objects.get_mut(&container).unwrap().add_occupant(shooter));
    world.objects.get_mut(&shooter).unwrap().contained_by = Some(container);
    world.apply_helix_contain_on_enter(container, shooter);
    assert!(world.objects[&shooter].weapon_bonus_garrisoned);
    world.objects.get_mut(&container).unwrap().status.effectively_dead = true;
    world.frame = 35;
    let radius = world.objects[&container].geometry_info().bounding_circle_radius();
    let expected = game_engine::common::random_value::SessionRandom::from_state(world.logic_random.snapshot());
    expected.next_real(0.0, std::f32::consts::TAU);
    expected.next_real(radius, radius + radius / 2.0);
    let delay = expected.next_int(30, 90) as u32;
    assert!(world.remove_owned_occupant_with_bonus_callback(container, shooter, &mut Default::default()));
    assert!(!world.objects[&shooter].weapon_bonus_garrisoned);
    assert_eq!(world.objects[&shooter].contained_by, Some(container));
    assert_eq!(world.objects[&shooter].weapon.as_ref().unwrap().next_fire_frame, 35 + delay);
    assert_eq!(world.logic_random.seed_words(), expected.seed_words());
}

#[test]
fn authored_shared_status_restarts_empty_sibling_until_exact_expiry() {
    let (mut game, shooter, target) = authored_clock_world(30, 30, 2, true);
    let secondary = game.objects.get_mut(&shooter).unwrap().secondary_weapon.as_mut().unwrap();
    Object::set_weapon_clip_percent_full(secondary, 0.0, true, 0);
    game.frame = 15;
    assert!(game.fire_owned_passive_weapon(shooter, target, 0, Vec3::ZERO));
    let secondary = game.objects[&shooter].secondary_weapon.as_ref().unwrap();
    assert_eq!(secondary.frame_status_at(16), WeaponFireStatus::BetweenFiringShots);
    game.objects.get_mut(&shooter).unwrap().on_owned_weapon_bonus_change(35, &[1.0; 3], &game.logic_random);
    let secondary = game.objects[&shooter].secondary_weapon.as_ref().unwrap();
    assert_eq!(secondary.next_fire_frame, 215);
    assert_eq!(secondary.frame_status_at(214), WeaponFireStatus::ReloadingClip);
    assert_eq!(secondary.frame_status_at(215), WeaponFireStatus::OutOfAmmo);
}
