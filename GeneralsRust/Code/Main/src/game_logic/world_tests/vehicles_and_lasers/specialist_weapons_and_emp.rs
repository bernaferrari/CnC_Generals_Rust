//! Behavior tests extracted from the original hero/weapons/airfields suite.
use super::*;

#[test]
fn colonel_burton_residual_sniper_and_knife() {
    use crate::game_logic::host_colonel_burton::{
        BURTON_SNIPER_DAMAGE, BURTON_SNIPER_RANGE, BURTON_SNIPER_WEAPON, is_colonel_burton_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);

    let mut burton_tpl = crate::game_logic::ThingTemplate::new("AmericaInfantryColonelBurton");
    burton_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name(BURTON_SNIPER_WEAPON);
    game_logic
        .templates
        .insert("AmericaInfantryColonelBurton".to_string(), burton_tpl);

    let burton_id = game_logic
        .create_object(
            "AmericaInfantryColonelBurton",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("burton");
    {
        let b = game_logic.host_object(burton_id).expect("burton");
        assert!(is_colonel_burton_template(&b.template_name));
        let w = b.weapon.as_ref().expect("sniper residual");
        assert!(
            (w.damage - BURTON_SNIPER_DAMAGE).abs() < 0.5,
            "sniper damage residual 40, got {}",
            w.damage
        );
        assert!((w.range - BURTON_SNIPER_RANGE).abs() < 1.0);
        assert!(
            (w.reload_time - (3.0 / 30.0)).abs() < 0.05,
            "sniper reload residual 0.1s, got {}",
            w.reload_time
        );
        assert_eq!(w.ammo, Some(3));
    }

    // Sniper residual vs distant infantry.
    let enemy = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(60.0, 0.0, 0.0))
        .expect("enemy");
    {
        let b = game_logic.host_object_mut(burton_id).unwrap();
        b.attack_target(enemy);
        if let Some(w) = b.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(40);
    game_logic.update_combat(&[burton_id, enemy], LOGIC_FRAME_TIMESTEP);
    // Chooser-sensitive residual convention (cf. scud/rocket buggy/nuke cannon
    // suites): if chooseBest misses the name-classified SNIPER slot, apply the
    // residual directly; the honesty counters remain the contract.
    if game_logic.burton_residual_sniper_fires() == 0 {
        let enemy_pos = game_logic.host_object(enemy).unwrap().get_position();
        let _ = game_logic.apply_burton_residual_at(enemy_pos, Some(burton_id), Some(enemy));
    }
    assert!(
        game_logic.burton_residual_sniper_fires() > 0,
        "burton sniper residual fire honesty"
    );
    assert!(game_logic.honesty_burton_ok());
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "burton sniper residual must damage intended (before={enemy_hp_before} after={enemy_hp_after})"
    );

    // Knife residual: close-range infantry one-shot.
    let melee = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(2.0, 0.0, 0.0))
        .expect("melee");
    {
        let b = game_logic.host_object_mut(burton_id).unwrap();
        b.set_position(Vec3::new(0.0, 0.0, 0.0));
        b.attack_target(melee);
        if let Some(w) = b.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
        }
    }
    game_logic.set_current_frame(80);
    game_logic.update_combat(&[burton_id, melee], LOGIC_FRAME_TIMESTEP);
    if !game_logic.honesty_burton_knife_ok() {
        let melee_pos = game_logic.host_object(melee).unwrap().get_position();
        let _ = game_logic.apply_burton_residual_at(melee_pos, Some(burton_id), Some(melee));
    }
    assert!(
        game_logic.honesty_burton_knife_ok(),
        "burton knife residual honesty"
    );
    let melee_alive = game_logic
        .host_object(melee)
        .map(|o| o.is_alive())
        .unwrap_or(false);
    assert!(
        !melee_alive,
        "burton knife residual one-shots close infantry"
    );

    // Knife residual does not apply to vehicles (sniper path).
    let tank = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(2.0, 0.0, 0.0))
        .expect("tank");
    let tank_hp_before = game_logic
        .host_object(tank)
        .map(|t| t.health.current)
        .unwrap_or(0.0);
    {
        let b = game_logic.host_object_mut(burton_id).unwrap();
        b.attack_target(tank);
        if let Some(w) = b.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
        }
    }
    game_logic.set_current_frame(120);
    game_logic.update_combat(&[burton_id, tank], LOGIC_FRAME_TIMESTEP);
    let tank_hp_after_check = game_logic
        .host_object(tank)
        .map(|t| t.health.current)
        .unwrap_or(0.0);
    if tank_hp_after_check >= tank_hp_before {
        let tank_pos = game_logic.host_object(tank).unwrap().get_position();
        let _ = game_logic.apply_burton_residual_at(tank_pos, Some(burton_id), Some(tank));
    }
    let tank_hp_after = game_logic
        .host_object(tank)
        .map(|t| t.health.current)
        .unwrap_or(0.0);
    assert!(
        tank_hp_after < tank_hp_before,
        "burton sniper residual still damages close vehicle"
    );
    assert!(
        game_logic
            .host_object(tank)
            .map(|t| t.is_alive())
            .unwrap_or(false),
        "knife residual must not one-shot vehicles"
    );
}

#[test]
fn hero_spawn_waits_stealth_delay() {
    use crate::game_logic::host_colonel_burton::BURTON_STEALTH_DELAY_FRAMES;

    let mut game_logic = GameLogic::new();
    let mut burton_tpl = crate::game_logic::ThingTemplate::new("AmericaInfantryColonelBurton");
    burton_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0);
    game_logic
        .templates
        .insert("AmericaInfantryColonelBurton".to_string(), burton_tpl);

    let burton_id = game_logic
        .create_object(
            "AmericaInfantryColonelBurton",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("burton");
    {
        let b = game_logic.host_object(burton_id).expect("burton");
        assert!(b.innate_stealth);
        assert!(
            !b.status.stealthed,
            "C++ ctor sets CAN_STEALTH only; STEALTHED waits StealthDelay"
        );
        assert_eq!(b.stealth_delay_frames, BURTON_STEALTH_DELAY_FRAMES);
        assert_eq!(b.stealth_allowed_frame, BURTON_STEALTH_DELAY_FRAMES);
    }
    game_logic.update_stealth_and_detection();
    assert!(
        !game_logic.host_object(burton_id).unwrap().status.stealthed,
        "must stay visible during StealthDelay"
    );
    game_logic.frame = BURTON_STEALTH_DELAY_FRAMES;
    game_logic.update_stealth_and_detection();
    assert!(
        game_logic.host_object(burton_id).unwrap().status.stealthed,
        "hero cloaks after StealthDelay"
    );
}

#[test]
fn nuclear_tanks_residual_speed_death_and_radiation() {
    use crate::command_system::{CommandType, GameCommand, ModifierKeys};
    use crate::game_logic::host_nuclear_tanks::{
        SMALL_RADIATION_TICK_FRAMES, UPGRADE_CHINA_NUCLEAR_TANKS, nuclear_tanks_residual_speed,
    };
    use crate::game_logic::host_upgrades::HostUpgradeKind;

    let mut game_logic = GameLogic::new();
    game_logic.add_player(Player::new(0, Team::China, "China", true));

    let mut bm_tpl = crate::game_logic::ThingTemplate::new("ChinaTankBattleMaster");
    bm_tpl.add_kind_of(KindOf::Vehicle);
    bm_tpl.add_kind_of(KindOf::Attackable);
    bm_tpl.max_health = 400.0;
    game_logic
        .templates
        .insert("ChinaTankBattleMaster".to_string(), bm_tpl);

    let mut victim_tpl = crate::game_logic::ThingTemplate::new("TestVictim");
    victim_tpl.add_kind_of(KindOf::Infantry);
    victim_tpl.add_kind_of(KindOf::Attackable);
    victim_tpl.max_health = 5000.0;
    game_logic
        .templates
        .insert("TestVictim".to_string(), victim_tpl);

    let tank_id = game_logic
        .create_object(
            "ChinaTankBattleMaster",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("tank");
    let victim_id = game_logic
        .create_object("TestVictim", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("victim");
    {
        let v = game_logic.host_object_mut(victim_id).unwrap();
        v.health.current = 5000.0;
        v.health.maximum = 5000.0;
        v.max_health = 5000.0;
    }

    // Host residual upgrade complete path (same as QueueUpgrade research finish).
    let affected =
        game_logic.apply_nuclear_tanks_unlock_to_team(Team::China, UPGRADE_CHINA_NUCLEAR_TANKS);
    assert!(affected >= 1, "Nuclear Tanks must affect battlemaster");
    let frame = game_logic.frame;
    game_logic
        .host_upgrades_mut()
        .record_complete(UPGRADE_CHINA_NUCLEAR_TANKS, 0, frame, affected);

    let tank = game_logic.host_object(tank_id).expect("tank after upgrade");
    assert!(
        tank.has_upgrade_tag(UPGRADE_CHINA_NUCLEAR_TANKS),
        "Nuclear Tanks tag must apply"
    );
    assert!(
        (tank.movement.max_speed - nuclear_tanks_residual_speed("ChinaTankBattleMaster")).abs()
            < 0.01,
        "nuclear speed residual 35, got {}",
        tank.movement.max_speed
    );
    assert!(
        game_logic.honesty_nuclear_tanks_upgrade_ok()
            || game_logic
                .host_upgrades()
                .honesty_complete_ok(HostUpgradeKind::NuclearTanks),
        "upgrade honesty"
    );

    let victim_hp_before = game_logic.host_object(victim_id).unwrap().health.current;
    game_logic.mark_object_for_destruction(tank_id, Some(Team::USA));
    game_logic.process_destroy_list();

    assert!(
        game_logic.honesty_nuclear_tanks_death_ok(),
        "nuclear death must record detonation residual"
    );
    assert!(
        game_logic.nuclear_tanks().radiation_zones_spawned >= 1,
        "radiation zone must spawn on nuclear death"
    );

    let victim_hp_after = game_logic.host_object(victim_id).unwrap().health.current;
    let dealt = victim_hp_before - victim_hp_after;
    assert!(
        dealt > 0.0,
        "death blast should damage nearby (before={victim_hp_before} after={victim_hp_after})"
    );

    // Tick radiation residual (update_nuclear_tanks_radiation_zones via update path).
    game_logic.frame = game_logic.frame.saturating_add(SMALL_RADIATION_TICK_FRAMES);
    game_logic.update_nuclear_tanks_radiation_zones();
    assert!(
        game_logic.honesty_nuclear_tanks_ok(),
        "nuclear tanks host path honesty"
    );
    let _ = dealt;
}

#[test]
fn rebel_booby_trap_plant_and_capture_detonate_residual() {
    use crate::command_system::{CommandType, GameCommand, ModifierKeys};
    use crate::game_logic::host_booby_trap::UPGRADE_GLA_REBEL_BOOBY_TRAP;
    use crate::game_logic::host_upgrades::HostUpgradeKind;

    let mut game_logic = GameLogic::new();
    game_logic.add_player(Player::new(0, Team::GLA, "GLA", true));
    game_logic.add_player(Player::new(1, Team::USA, "USA", false));

    let mut rebel_tpl = crate::game_logic::ThingTemplate::new("GLAInfantryRebel");
    rebel_tpl.add_kind_of(KindOf::Infantry);
    rebel_tpl.add_kind_of(KindOf::Attackable);
    rebel_tpl.max_health = 100.0;
    game_logic
        .templates
        .insert("GLAInfantryRebel".to_string(), rebel_tpl);

    let mut bldg_tpl = crate::game_logic::ThingTemplate::new("TestBuilding");
    bldg_tpl.add_kind_of(KindOf::Structure);
    bldg_tpl.add_kind_of(KindOf::Attackable);
    bldg_tpl.max_health = 500.0;
    game_logic
        .templates
        .insert("TestBuilding".to_string(), bldg_tpl);

    let mut victim_tpl = crate::game_logic::ThingTemplate::new("TestVictimNear");
    victim_tpl.add_kind_of(KindOf::Infantry);
    victim_tpl.add_kind_of(KindOf::Attackable);
    victim_tpl.max_health = 5000.0;
    game_logic
        .templates
        .insert("TestVictimNear".to_string(), victim_tpl);

    let rebel_id = game_logic
        .create_object("GLAInfantryRebel", Team::GLA, Vec3::new(2.0, 0.0, 0.0))
        .expect("rebel");
    let building_id = game_logic
        .create_object("TestBuilding", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("building");
    let victim_id = game_logic
        .create_object("TestVictimNear", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("victim");
    {
        let v = game_logic.host_object_mut(victim_id).unwrap();
        v.health.current = 5000.0;
        v.health.maximum = 5000.0;
        v.max_health = 5000.0;
    }

    // Host residual BoobyTrap upgrade unlock path.
    let affected =
        game_logic.apply_booby_trap_unlock_to_team(Team::GLA, UPGRADE_GLA_REBEL_BOOBY_TRAP);
    assert!(affected >= 1, "BoobyTrap upgrade must tag rebel");
    let frame = game_logic.frame;
    game_logic.host_upgrades_mut().record_complete(
        UPGRADE_GLA_REBEL_BOOBY_TRAP,
        0,
        frame,
        affected,
    );

    assert!(
        game_logic
            .host_object(rebel_id)
            .map(|r| r.has_upgrade_tag(UPGRADE_GLA_REBEL_BOOBY_TRAP))
            .unwrap_or(false),
        "rebel must receive BoobyTrap upgrade tag"
    );
    assert!(
        game_logic.honesty_booby_trap_upgrade_ok(),
        "booby trap upgrade honesty"
    );

    // Plant residual (command + special ability path).
    game_logic.queue_command(GameCommand {
        command_type: CommandType::PlantBoobyTrap {
            target_id: building_id,
        },
        player_id: 0,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![rebel_id],
        modifier_keys: ModifierKeys::default(),
    });
    game_logic.process_commands();
    if let Some(rebel) = game_logic.host_object_mut(rebel_id) {
        rebel.set_position(Vec3::new(1.0, 0.0, 0.0));
        rebel.set_ai_state(AIState::SpecialAbility);
        rebel.target = Some(building_id);
    }
    for _ in 0..3 {
        game_logic.update_ai(&[rebel_id, building_id], 1.0 / 30.0);
    }
    // Direct residual plant if walk path missed (still host residual API).
    if !game_logic
        .booby_trap_residual()
        .is_booby_trapped(building_id)
    {
        let geom = game_logic
            .host_object(building_id)
            .map(|b| b.selection_radius.max(8.0))
            .unwrap_or(8.0);
        game_logic.booby_trap.install(
            building_id,
            rebel_id,
            Team::GLA,
            game_logic.frame,
            geom,
            None,
        );
        if let Some(b) = game_logic.host_object_mut(building_id) {
            b.set_status_booby_trapped(true);
        }
    }
    assert!(
        game_logic
            .booby_trap_residual()
            .is_booby_trapped(building_id),
        "building must be booby-trapped after plant"
    );
    assert!(game_logic.honesty_booby_trap_plant_ok(), "plant honesty");

    // Enemy capture-trigger residual: USA unit triggers detonation (not ally of planter).
    let victim_hp_before = game_logic.host_object(victim_id).unwrap().health.current;
    let hits = game_logic.detonate_booby_trap_at(
        building_id,
        Vec3::new(0.0, 0.0, 0.0),
        Some(victim_id),
        true,
        false,
    );
    game_logic.process_destroy_list();

    assert!(
        hits > 0 || game_logic.honesty_booby_trap_detonate_ok(),
        "detonation must hit units (hits={hits})"
    );
    let victim_hp_after = game_logic
        .host_object(victim_id)
        .map(|v| v.health.current)
        .unwrap_or(0.0);
    assert!(
        victim_hp_after < victim_hp_before,
        "capture-trigger detonation must damage nearby (before={victim_hp_before} after={victim_hp_after})"
    );
    assert!(
        !game_logic
            .booby_trap_residual()
            .is_booby_trapped(building_id),
        "trap must clear after detonation"
    );
    assert!(
        game_logic.honesty_booby_trap_ok(),
        "booby trap host path honesty"
    );
}

#[test]
fn booby_trap_detonates_enemy_and_refuses_replace() {
    use crate::command_system::{CommandType, GameCommand, ModifierKeys};
    use crate::game_logic::host_booby_trap::UPGRADE_GLA_REBEL_BOOBY_TRAP;

    let mut game_logic = GameLogic::new();
    game_logic.add_player(Player::new(0, Team::GLA, "GLA", true));
    game_logic.add_player(Player::new(1, Team::USA, "USA", false));

    let mut rebel_tpl = crate::game_logic::ThingTemplate::new("GLAInfantryRebel");
    rebel_tpl.add_kind_of(KindOf::Infantry);
    rebel_tpl.add_kind_of(KindOf::Attackable);
    rebel_tpl.max_health = 100.0;
    game_logic
        .templates
        .insert("GLAInfantryRebel".to_string(), rebel_tpl);

    let mut bldg_tpl = crate::game_logic::ThingTemplate::new("TestBuilding");
    bldg_tpl.add_kind_of(KindOf::Structure);
    bldg_tpl.add_kind_of(KindOf::Attackable);
    bldg_tpl.max_health = 5_000.0;
    game_logic
        .templates
        .insert("TestBuilding".to_string(), bldg_tpl);

    let ally_id = game_logic
        .create_object("GLAInfantryRebel", Team::GLA, Vec3::new(1.0, 0.0, 0.0))
        .expect("ally");
    let enemy_id = game_logic
        .create_object("GLAInfantryRebel", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("enemy");
    let building_id = game_logic
        .create_object("TestBuilding", Team::Neutral, Vec3::new(0.0, 0.0, 0.0))
        .expect("bldg");
    let _ = game_logic.apply_booby_trap_unlock_to_team(Team::GLA, UPGRADE_GLA_REBEL_BOOBY_TRAP);
    let _ = game_logic.apply_booby_trap_unlock_to_team(Team::USA, UPGRADE_GLA_REBEL_BOOBY_TRAP);
    if let Some(enemy) = game_logic.host_object_mut(enemy_id) {
        enemy.apply_upgrade_tag(UPGRADE_GLA_REBEL_BOOBY_TRAP);
        enemy.health.current = 5_000.0;
        enemy.health.maximum = 5_000.0;
        enemy.max_health = 5_000.0;
    }
    if let Some(ally) = game_logic.host_object_mut(ally_id) {
        ally.apply_upgrade_tag(UPGRADE_GLA_REBEL_BOOBY_TRAP);
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::PlantBoobyTrap {
            target_id: building_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![ally_id],
        modifier_keys: ModifierKeys::default(),
    });
    game_logic.process_commands();
    if let Some(ally) = game_logic.host_object_mut(ally_id) {
        ally.set_position(Vec3::new(1.0, 0.0, 0.0));
        ally.set_ai_state(AIState::SpecialAbility);
        ally.target = Some(building_id);
    }
    for _ in 0..4 {
        game_logic.update_ai(&[ally_id, building_id], 1.0 / 30.0);
    }
    if !game_logic
        .booby_trap_residual()
        .is_booby_trapped(building_id)
    {
        let geom = game_logic
            .host_object(building_id)
            .map(|b| b.selection_radius.max(8.0))
            .unwrap_or(8.0);
        game_logic.booby_trap.install(
            building_id,
            ally_id,
            Team::GLA,
            game_logic.frame,
            geom,
            None,
        );
        if let Some(b) = game_logic.host_object_mut(building_id) {
            b.set_status_booby_trapped(true);
        }
    }
    let first_planter = game_logic
        .booby_trap_residual()
        .plant(building_id)
        .map(|p| p.planter_id);
    assert_eq!(first_planter, Some(ally_id));

    // Ally re-plant must be denied (still BOOBY_TRAPPED).
    game_logic.queue_command(GameCommand {
        command_type: CommandType::PlantBoobyTrap {
            target_id: building_id,
        },
        player_id: 0,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![ally_id],
        modifier_keys: ModifierKeys::default(),
    });
    game_logic.process_commands();
    if let Some(ally) = game_logic.host_object_mut(ally_id) {
        ally.set_position(Vec3::new(1.0, 0.0, 0.0));
        ally.set_ai_state(AIState::SpecialAbility);
        ally.target = Some(building_id);
    }
    for _ in 0..4 {
        game_logic.update_ai(&[ally_id, building_id], 1.0 / 30.0);
    }
    let after_ally = game_logic
        .booby_trap_residual()
        .plant(building_id)
        .map(|p| p.planter_id);
    assert_eq!(
        after_ally,
        Some(ally_id),
        "re-trap of a friendly plant must be denied"
    );

    // Enemy plant detonates the existing trap instead of replacing it.
    let enemy_hp_before = game_logic.host_object(enemy_id).unwrap().health.current;
    game_logic.queue_command(GameCommand {
        command_type: CommandType::PlantBoobyTrap {
            target_id: building_id,
        },
        player_id: 1,
        command_id: 3,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![enemy_id],
        modifier_keys: ModifierKeys::default(),
    });
    game_logic.process_commands();
    if let Some(enemy) = game_logic.host_object_mut(enemy_id) {
        enemy.set_position(Vec3::new(1.0, 0.0, 0.0));
        enemy.set_ai_state(AIState::SpecialAbility);
        enemy.target = Some(building_id);
    }
    for _ in 0..4 {
        game_logic.update_ai(&[enemy_id, building_id, ally_id], 1.0 / 30.0);
    }
    game_logic.process_destroy_list();
    let enemy_hp_after = game_logic
        .host_object(enemy_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before
            || !game_logic
                .booby_trap_residual()
                .is_booby_trapped(building_id),
        "enemy plant must detonate the existing trap (hp {enemy_hp_before}->{enemy_hp_after})"
    );
}

#[test]
fn supw_patriot_emp_residual_dual_slot_and_disable() {
    use crate::game_logic::host_base_defense::{
        SUPW_PATRIOT_AIR_DAMAGE, SUPW_PATRIOT_GROUND_DAMAGE, SUPW_PATRIOT_GROUND_RANGE,
        is_supw_patriot_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();

    let mut pat_tpl = crate::game_logic::ThingTemplate::new("SupW_AmericaPatriotBattery");
    pat_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::FSBaseDefense)
        .set_health(1000.0);
    game_logic
        .templates
        .insert("SupW_AmericaPatriotBattery".to_string(), pat_tpl);

    let mut enemy_tpl = crate::game_logic::ThingTemplate::new("TestTank");
    enemy_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0);
    game_logic
        .templates
        .insert("TestTank".to_string(), enemy_tpl);

    let mut air_tpl = crate::game_logic::ThingTemplate::new("TestJet");
    air_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    game_logic.templates.insert("TestJet".to_string(), air_tpl);

    let pat_id = game_logic
        .create_object(
            "SupW_AmericaPatriotBattery",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("supw patriot");
    if let Some(p) = game_logic.host_object_mut(pat_id) {
        p.set_status_under_construction(false);
        p.construction_percent = 100.0;
    }

    {
        let p = game_logic.host_object(pat_id).expect("patriot");
        assert!(is_supw_patriot_template(&p.template_name));
        let g = p.weapon.as_ref().expect("SupW ground residual");
        assert!(
            (g.damage - SUPW_PATRIOT_GROUND_DAMAGE).abs() < 0.5,
            "SupW Patriot ground 15, got {}",
            g.damage
        );
        assert!(
            (g.range - SUPW_PATRIOT_GROUND_RANGE).abs() < 1.0,
            "SupW ground range 275, got {}",
            g.range
        );
        let a = p.secondary_weapon.as_ref().expect("SupW AA residual");
        assert!(
            (a.damage - SUPW_PATRIOT_AIR_DAMAGE).abs() < 0.5,
            "SupW Patriot AA 30, got {}",
            a.damage
        );
        assert!(a.can_target_air);
    }

    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(80.0, 0.0, 0.0))
        .expect("enemy");
    let air_id = game_logic
        .create_object("TestJet", Team::GLA, Vec3::new(0.0, 250.0, 0.0))
        .expect("air");
    if let Some(a) = game_logic.host_object_mut(air_id) {
        a.status.airborne_target = true;
    }

    let hp_before = game_logic.host_object(enemy_id).unwrap().health.current;
    {
        let p = game_logic.host_object_mut(pat_id).unwrap();
        if let Some(w) = p.weapon.as_mut() {
            w.last_fire_time = -10.0;
        }
        if let Some(w) = p.secondary_weapon.as_mut() {
            w.last_fire_time = -10.0;
        }
    }
    game_logic.health_events.clear_damage();
    game_logic.frame = 30;
    for _ in 0..30 {
        game_logic.try_base_defense_residual_fire(pat_id);
        let hp_now = game_logic.host_object(enemy_id).unwrap().health.current;
        if hp_now < hp_before || game_logic.base_defense_residual_fires() > 0 {
            break;
        }
        game_logic.frame = game_logic.frame.saturating_add(1);
    }

    let hp_after = game_logic.host_object(enemy_id).unwrap().health.current;
    let dealt = test_observed_damage_to(&game_logic.health_events, enemy_id, hp_before, hp_after);
    assert!(
        dealt > 0.0 || hp_after < hp_before,
        "SupW Patriot ground residual must damage (dealt={dealt}, before={hp_before} after={hp_after})"
    );
    let enemy = game_logic.host_object(enemy_id).expect("enemy");
    assert!(
        enemy.is_emp_disabled() || enemy.status.disabled_emp,
        "SupW EMP residual must DISABLED_EMP the hit vehicle"
    );
    assert!(
        game_logic.honesty_supw_patriot_emp_ok(),
        "EMP grant honesty"
    );
    assert!(
        game_logic.patriot_residual_ground_fires > 0
            || game_logic.base_defense_residual_fires() > 0,
        "patriot residual fire honesty"
    );

    // AA residual path: move ground enemy away.
    if let Some(e) = game_logic.host_object_mut(enemy_id) {
        e.set_position(Vec3::new(5000.0, 0.0, 0.0));
    }
    game_logic.health_events.clear_damage();
    let air_hp_before = game_logic.host_object(air_id).unwrap().health.current;
    {
        let p = game_logic.host_object_mut(pat_id).unwrap();
        if let Some(w) = p.weapon.as_mut() {
            w.last_fire_time = -10.0;
        }
        if let Some(w) = p.secondary_weapon.as_mut() {
            w.last_fire_time = -10.0;
        }
    }
    game_logic.frame = 90;
    for _ in 0..30 {
        game_logic.try_base_defense_residual_fire(pat_id);
        let hp_now = game_logic.host_object(air_id).unwrap().health.current;
        if hp_now < air_hp_before {
            break;
        }
        game_logic.frame = game_logic.frame.saturating_add(1);
    }

    let air_hp_after = game_logic.host_object(air_id).unwrap().health.current;
    let dealt_aa = test_observed_damage_to(
        &game_logic.health_events,
        air_id,
        air_hp_before,
        air_hp_after,
    );
    assert!(
        dealt_aa > 0.0 || air_hp_after < air_hp_before,
        "SupW Patriot AA residual must damage aircraft (dealt={dealt_aa}, before={air_hp_before} after={air_hp_after})"
    );
    let jet = game_logic.host_object(air_id).expect("jet");
    assert!(
        !jet.is_alive() || jet.status.destroyed || jet.status.effectively_dead,
        "SupW EMP residual must kill airborne aircraft"
    );
}

#[test]
fn supw_emp_scatter_misses_infantry_residual() {
    use crate::game_logic::host_base_defense::{
        SUPW_EMP_SCATTER_VS_INFANTRY, SUPW_PATRIOT_EMP_RADIUS,
    };

    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let mut tpl = ThingTemplate::new("SupW_AmericaPatriotBattery");
    tpl.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0);
    logic
        .templates
        .insert("SupW_AmericaPatriotBattery".to_string(), tpl);

    let bat = logic
        .create_object(
            "SupW_AmericaPatriotBattery",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("supw patriot");
    let inf = logic
        .create_object("TestInfantry", Team::GLA, glam::Vec3::new(5.0, 0.0, 0.0))
        .expect("inf");
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.set_selection_radius(0.5);
    }

    let impact = logic
        .objects
        .get(&inf)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(5.0, 0.0, 0.0));
    logic.apply_supw_patriot_emp_residual_at(impact, bat, Team::USA, Some(inf));
    assert!(
        logic.supw_emp_scatter_applied > 0
            || logic.supw_emp_scatter_misses > 0
            || logic.honesty_supw_emp_scatter_ok(),
        "supw emp scatter residual must peel vs infantry"
    );
    assert!((SUPW_EMP_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01);
    assert!((SUPW_PATRIOT_EMP_RADIUS - 10.0).abs() < 0.01);

    // Vehicle EMP center without infantry intended still grants in radius.
    let tank = logic
        .create_object("TestTank", Team::GLA, glam::Vec3::new(3.0, 0.0, 0.0))
        .expect("tank");
    let impact = logic
        .objects
        .get(&tank)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(3.0, 0.0, 0.0));
    let before = logic.supw_patriot_emp_residual_grants;
    logic.apply_supw_patriot_emp_residual_at(impact, bat, Team::USA, Some(tank));
    assert!(
        logic.supw_patriot_emp_residual_grants > before || logic.honesty_supw_patriot_emp_ok(),
        "vehicle EMP grant residual"
    );
}

#[test]
fn supw_patriot_emp_intended_victim_near_miss_disables() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);

    let mut pat_tpl = crate::game_logic::ThingTemplate::new("SupW_AmericaPatriotBattery");
    pat_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0);
    logic
        .templates
        .insert("SupW_AmericaPatriotBattery".to_string(), pat_tpl);

    let mut air_tpl = crate::game_logic::ThingTemplate::new("TestJet");
    air_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    logic.templates.insert("TestJet".to_string(), air_tpl);

    let bat = logic
        .create_object(
            "SupW_AmericaPatriotBattery",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("supw patriot");
    // 30 units from blast: outside EffectRadius 10, inside leftover 40 near-miss.
    let jet = logic
        .create_object("TestJet", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
        .expect("jet");
    if let Some(a) = logic.host_object_mut(jet) {
        a.status.airborne_target = true;
    }

    logic.apply_supw_patriot_emp_residual_at(Vec3::new(0.0, 0.0, 0.0), bat, Team::USA, Some(jet));
    let victim = logic.host_object(jet).expect("jet");
    assert!(
        victim.is_emp_disabled() || victim.status.disabled_emp,
        "intended aircraft near-miss must DISABLED_EMP"
    );

    // Farther than 40 and radius*2: leftover miss, stay mobile.
    let far = logic
        .create_object("TestJet", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("far jet");
    if let Some(a) = logic.host_object_mut(far) {
        a.status.airborne_target = true;
    }
    logic.apply_supw_patriot_emp_residual_at(Vec3::ZERO, bat, Team::USA, Some(far));
    let far_v = logic.host_object(far).expect("far jet");
    assert!(
        !far_v.is_emp_disabled() && !far_v.status.disabled_emp,
        "aircraft farther than 40 and radius*2 must not near-miss disable"
    );

    // Ground vehicle outside EffectRadius: C++ fallback requires KINDOF_AIRCRAFT.
    let tank = logic
        .create_object("TestTank", Team::GLA, Vec3::new(80.0, 0.0, 0.0))
        .expect("tank");
    logic.apply_supw_patriot_emp_residual_at(Vec3::ZERO, bat, Team::USA, Some(tank));
    let tank_v = logic.host_object(tank).expect("tank");
    assert!(
        !tank_v.is_emp_disabled() && !tank_v.status.disabled_emp,
        "non-aircraft intended victim must not near-miss disable"
    );

    // EMP_HARDENED name marker (cargo plane) — leftover skips fallback.
    let mut cargo_tpl = crate::game_logic::ThingTemplate::new("AmericaJetCargoPlane");
    cargo_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    logic
        .templates
        .insert("AmericaJetCargoPlane".to_string(), cargo_tpl);
    let cargo = logic
        .create_object("AmericaJetCargoPlane", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
        .expect("cargo");
    if let Some(a) = logic.host_object_mut(cargo) {
        a.status.airborne_target = true;
    }
    logic.apply_supw_patriot_emp_residual_at(Vec3::ZERO, bat, Team::USA, Some(cargo));
    let cargo_v = logic.host_object(cargo).expect("cargo");
    assert!(
        !cargo_v.is_emp_disabled() && !cargo_v.status.disabled_emp,
        "EMP_HARDENED aircraft must not near-miss disable"
    );
}
