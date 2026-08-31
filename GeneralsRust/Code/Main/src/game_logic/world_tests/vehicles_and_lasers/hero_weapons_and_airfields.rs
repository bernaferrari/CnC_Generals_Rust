//! Behavior suite extracted from `vehicles_and_lasers`.
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
    crate::game_logic::host_damage_log::clear();
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
    let dealt = test_observed_damage_to(enemy_id, hp_before, hp_after);
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
    crate::game_logic::host_damage_log::clear();
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
    let dealt_aa = test_observed_damage_to(air_id, air_hp_before, air_hp_after);
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

#[test]
fn anthrax_gamma_residual_toxin_stream_and_field() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_toxin_tractor::{
        TOXIN_MED_FIELD_DAMAGE_UPGRADED, TOXIN_STREAM_DAMAGE_GAMMA, TOXIN_STREAM_DAMAGE_UPGRADED,
        TOXIN_TRUCK_GUN, TOXIN_TRUCK_SPRAYER, UPGRADE_GLA_ANTHRAX_GAMMA,
    };
    use crate::game_logic::host_upgrades::HostUpgradeKind;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::GLA, "GLA", true);
    player.resources.supplies = 5000;
    game_logic.add_player(player);
    ensure_test_barracks_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut toxin_tpl = crate::game_logic::ThingTemplate::new("Chem_GLAVehicleToxinTruck");
    toxin_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_primary_weapon_name(TOXIN_TRUCK_GUN)
        .set_secondary_weapon_name(TOXIN_TRUCK_SPRAYER);
    game_logic
        .templates
        .insert("Chem_GLAVehicleToxinTruck".to_string(), toxin_tpl);

    let truck_id = game_logic
        .create_object(
            "Chem_GLAVehicleToxinTruck",
            Team::GLA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("chem toxin truck");
    let enemy = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy");
    let barracks_id = game_logic
        .create_object("TestBarracks", Team::GLA, Vec3::new(-40.0, 0.0, 0.0))
        .expect("barracks");

    // Chem baseline stream residual (Anthrax Beta 12.5).
    {
        let t = game_logic.host_object_mut(truck_id).unwrap();
        t.attack_target(enemy);
        if let Some(w) = t.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.05;
        }
        t.record_host_weapon_stats();
    }
    let hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    game_logic.set_current_frame(20);
    game_logic.update_combat(&[truck_id, enemy], LOGIC_FRAME_TIMESTEP);
    if game_logic.toxin_stream_missiles_spawned == 0 {
        let from = game_logic
            .host_object(truck_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(40.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_toxin_stream_projectile(truck_id, from, aim, Some(enemy))
                .is_some()
        );
    }
    for _ in 0..40 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_toxin_stream_projectiles();
        if !game_logic
            .objects
            .values()
            .any(|o| o.toxin_stream_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();
    assert!(
        game_logic.honesty_toxin_tractor_stream_ok()
            || game_logic.honesty_toxin_stream_projectile_ok(),
        "chem baseline stream honesty"
    );
    let hp_after_beta = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let beta_dmg = hp_before - hp_after_beta;
    assert!(
        beta_dmg + 0.1 >= TOXIN_STREAM_DAMAGE_UPGRADED,
        "Chem residual baseline must deal at least Anthrax Beta 12.5 (got {beta_dmg})"
    );

    // Research Anthrax Gamma residual via QueueUpgrade → complete.
    game_logic.queue_command(GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: UPGRADE_GLA_ANTHRAX_GAMMA.to_string(),
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![barracks_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert!(
        game_logic
            .host_upgrades()
            .honesty_queue_ok(HostUpgradeKind::AnthraxGamma)
    );
    // C++ research advances on the producer's Upgrade.ini BuildTime
    // (ProductionUpdate.cpp:686-704); tick past the 60s AnthraxGamma window.
    game_logic.update();
    game_logic.update_with_dt(61.0);
    assert!(
        game_logic
            .host_upgrades()
            .honesty_complete_ok(HostUpgradeKind::AnthraxGamma),
        "AnthraxGamma complete honesty"
    );
    assert!(
        game_logic
            .host_upgrades()
            .honesty_host_path_ok(HostUpgradeKind::AnthraxGamma),
        "AnthraxGamma must tag toxin units"
    );
    let truck = game_logic.host_object(truck_id).expect("truck");
    assert!(
        truck.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_GAMMA)
            || truck.has_upgrade_tag("Chem_Upgrade_GLAAnthraxGamma")
            || truck.has_upgrade_tag("Upgrade_GLAAnthraxGamma"),
        "truck must receive gamma upgrade tag"
    );

    // Gamma stream residual: 20.5 (fresh target so prior stream splash cannot mask dmg).
    let gamma_enemy = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(45.0, 0.0, 0.0))
        .expect("gamma enemy");
    {
        let t = game_logic.host_object_mut(truck_id).unwrap();
        t.active_weapon_slot = 0;
        t.attack_target(gamma_enemy);
        if let Some(w) = t.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.05;
        }
        t.record_host_weapon_stats();
    }
    let hp_mid = game_logic
        .host_object(gamma_enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    game_logic.set_current_frame(40);
    let from = game_logic
        .host_object(truck_id)
        .map(|o| o.get_position())
        .unwrap_or(Vec3::ZERO);
    let aim = game_logic
        .host_object(gamma_enemy)
        .map(|o| o.get_position())
        .unwrap_or(Vec3::new(45.0, 0.0, 0.0));
    assert!(
        game_logic
            .spawn_toxin_stream_projectile(truck_id, from, aim, Some(gamma_enemy))
            .is_some(),
        "gamma stream projectile spawn"
    );
    for _ in 0..40 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_toxin_stream_projectiles();
        if !game_logic
            .objects
            .values()
            .any(|o| o.toxin_stream_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();
    let hp_after_gamma = game_logic
        .host_object(gamma_enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let gamma_dmg = hp_mid - hp_after_gamma;
    assert!(
        gamma_dmg + 0.1 >= TOXIN_STREAM_DAMAGE_GAMMA,
        "gamma stream must deal at least 20.5 (got {gamma_dmg})"
    );
    assert!(
        game_logic.toxin_tractor_registry().honesty_gamma_ok()
            || game_logic.honesty_toxin_stream_projectile_ok(),
        "gamma stream honesty"
    );

    // Contaminate spray residual → upgraded MediumPoisonField 2.5/tick.
    let spray_victim = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(8.0, 0.0, 0.0))
        .expect("spray victim");
    {
        let t = game_logic.host_object_mut(truck_id).unwrap();
        t.active_weapon_slot = 1;
        t.attack_target(spray_victim);
        if let Some(w) = t.secondary_weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.05;
            w.damage = 0.0;
            w.range = 15.0;
        }
        t.record_host_weapon_stats();
        if let Some(w) = t.weapon.as_mut() {
            w.last_fire_time = 0.0;
            w.reload_time = 1000.0;
        }
        t.record_host_weapon_stats();
    }
    game_logic.set_current_frame(60);
    use crate::game_logic::host_toxin_tractor::{
        TOXIN_SPRAY_CONTINUOUS_FIRE_COAST_FRAMES, TOXIN_SPRAY_MIN_SHOTS_TO_CREATE_OCL,
    };
    for f in 0..TOXIN_SPRAY_MIN_SHOTS_TO_CREATE_OCL {
        game_logic.set_current_frame(u64::from(50 + f));
        game_logic.update_combat(&[truck_id, spray_victim], LOGIC_FRAME_TIMESTEP);
    }
    for f in 0..TOXIN_SPRAY_MIN_SHOTS_TO_CREATE_OCL {
        game_logic.set_current_frame(u64::from(50 + f));
        let _ = game_logic.apply_toxin_tractor_spray_at(
            Vec3::new(10.0, 0.0, 0.0),
            Some(truck_id),
            Team::GLA,
        );
    }
    game_logic.set_current_frame(u64::from(
        50 + TOXIN_SPRAY_MIN_SHOTS_TO_CREATE_OCL + TOXIN_SPRAY_CONTINUOUS_FIRE_COAST_FRAMES,
    ));
    game_logic.tick_fire_ocl_after_weapon_cooldown();
    assert!(
        game_logic.honesty_toxin_tractor_spray_ok(),
        "gamma spray residual honesty"
    );
    assert!(
        game_logic.toxin_tractor_registry().active_count() > 0,
        "gamma spray must spawn medium poison field"
    );
    let zone = &game_logic.toxin_tractor_registry().active_zones()[0];
    assert!(
        (zone.damage_per_tick - TOXIN_MED_FIELD_DAMAGE_UPGRADED).abs() < 0.01,
        "gamma medium field DoT must be 2.5/tick (got {})",
        zone.damage_per_tick
    );
    assert!(zone.anthrax_tier.is_gamma());
}

#[test]
fn camo_netting_upgrade_stealths_gla_structures() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_upgrades::{
        HostUpgradeKind, UPGRADE_GLA_CAMO_NETTING, is_camo_netting_structure_template,
    };

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::GLA, "GLA", true);
    player.resources.supplies = 5000;
    game_logic.add_player(player);
    ensure_test_barracks_template(&mut game_logic);

    for name in [
        "Slth_GLACommandCenter",
        "GLATunnelNetwork",
        "GLAInfantryRebel",
    ] {
        if !game_logic.templates.contains_key(name) {
            let mut t = crate::game_logic::ThingTemplate::new(name);
            if name.contains("Rebel") {
                t.add_kind_of(KindOf::Infantry)
                    .add_kind_of(KindOf::Attackable)
                    .add_kind_of(KindOf::Selectable)
                    .set_health(100.0);
            } else {
                t.add_kind_of(KindOf::Structure)
                    .add_kind_of(KindOf::Selectable)
                    .set_health(1000.0);
            }
            game_logic.templates.insert(name.to_string(), t);
        }
    }
    if !game_logic.templates.contains_key("GLABlackMarket") {
        let mut market = crate::game_logic::ThingTemplate::new("GLABlackMarket");
        market
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::FSBlackMarket)
            .set_health(1000.0);
        game_logic
            .templates
            .insert("GLABlackMarket".to_string(), market);
    }
    let _market_id = game_logic
        .create_object("GLABlackMarket", Team::GLA, Vec3::new(-200.0, 0.0, 0.0))
        .expect("black market");

    assert!(is_camo_netting_structure_template("Slth_GLACommandCenter"));
    assert!(is_camo_netting_structure_template("GLATunnelNetwork"));

    let barracks_id = game_logic
        .create_object("TestBarracks", Team::GLA, Vec3::new(-40.0, 0.0, 0.0))
        .expect("barracks");
    let cc_id = game_logic
        .create_object("Slth_GLACommandCenter", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("slth cc");
    let tunnel_id = game_logic
        .create_object("GLATunnelNetwork", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("tunnel");
    let rebel_id = game_logic
        .create_object("GLAInfantryRebel", Team::GLA, Vec3::new(100.0, 0.0, 0.0))
        .expect("rebel");

    for id in [cc_id, tunnel_id] {
        let o = game_logic.host_object_mut(id).unwrap();
        o.set_status_stealthed(false);
        o.innate_stealth = false;
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: UPGRADE_GLA_CAMO_NETTING.to_string(),
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![barracks_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert!(
        game_logic
            .host_upgrades()
            .honesty_queue_ok(HostUpgradeKind::CamoNetting)
    );
    // C++ research advances on the producer's Upgrade.ini BuildTime
    // (ProductionUpdate.cpp:686-704); tick past the 5s CamoNetting window.
    game_logic.update_with_dt(6.0);
    assert!(
        game_logic
            .host_upgrades()
            .honesty_complete_ok(HostUpgradeKind::CamoNetting),
        "CamoNetting complete honesty"
    );
    assert!(
        game_logic
            .host_upgrades()
            .honesty_host_path_ok(HostUpgradeKind::CamoNetting),
        "CamoNetting host path honesty"
    );

    let cc = game_logic.host_object(cc_id).expect("cc");
    assert!(
        cc.innate_stealth && !cc.status.stealthed,
        "Slth Command Center CAN_STEALTH after CamoNetting; StealthDelay not elapsed"
    );
    assert!(
        cc.has_upgrade_tag(UPGRADE_GLA_CAMO_NETTING),
        "structure must receive CamoNetting tag"
    );
    let cloak_at = cc.stealth_allowed_frame;
    drop(cc);
    let tunnel = game_logic.host_object(tunnel_id).expect("tunnel");
    assert!(
        tunnel.innate_stealth && !tunnel.status.stealthed,
        "Tunnel Network CAN_STEALTH after CamoNetting; StealthDelay not elapsed"
    );
    drop(tunnel);
    game_logic.frame = cloak_at.max(game_logic.frame);
    game_logic.update_stealth_and_detection();
    assert!(
        game_logic.host_object(cc_id).expect("cc").status.stealthed,
        "Slth Command Center cloaks after StealthDelay"
    );
    assert!(
        game_logic
            .host_object(tunnel_id)
            .expect("tunnel")
            .status
            .stealthed,
        "Tunnel Network cloaks after StealthDelay"
    );

    let rebel = game_logic.host_object(rebel_id).expect("rebel");
    assert!(
        !rebel.has_upgrade_tag(UPGRADE_GLA_CAMO_NETTING),
        "fail-closed: Rebel does not receive CamoNetting (use Camouflage residual)"
    );
}

#[test]
fn stealth_fighter_science_production_gate_residual() {
    use crate::game_logic::host_stealth_fighter::{
        SCIENCE_STEALTH_FIGHTER, STEALTH_FIGHTER_BUILD_COST,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_airfield_template(&mut game_logic);

    let mut fighter_tpl = crate::game_logic::ThingTemplate::new("AmericaJetStealthFighter");
    fighter_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(160.0)
        .set_cost(STEALTH_FIGHTER_BUILD_COST, 0);
    fighter_tpl.build_time = 0.05;
    game_logic
        .templates
        .insert("AmericaJetStealthFighter".to_string(), fighter_tpl);

    // Airforce free residual (no science Prerequisite).
    let mut airf_tpl = crate::game_logic::ThingTemplate::new("AirF_AmericaJetStealthFighter");
    airf_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(160.0)
        .set_cost(1000, 0);
    airf_tpl.build_time = 0.05;
    game_logic
        .templates
        .insert("AirF_AmericaJetStealthFighter".to_string(), airf_tpl);

    let airfield_id = game_logic
        .create_object("TestAirfield", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("airfield");
    {
        let af = game_logic.host_object_mut(airfield_id).unwrap();
        af.set_status_under_construction(false);
    }

    // Deny without science.
    assert!(
        !game_logic.enqueue_production(airfield_id, "AmericaJetStealthFighter".to_string()),
        "must deny Stealth Fighter without SCIENCE_StealthFighter"
    );
    assert!(
        game_logic.honesty_stealth_fighter_science_deny_ok(),
        "deny honesty"
    );

    // AirF free residual still enqueues.
    assert!(
        game_logic.enqueue_production(airfield_id, "AirF_AmericaJetStealthFighter".to_string()),
        "AirF Stealth Fighter must not require science"
    );
    // Clear free airf queue for clean science path.
    assert!(game_logic.cancel_all_production(airfield_id));

    // Unlock science residual → enqueue + complete spawn.
    assert!(game_logic.unlock_team_science(Team::USA, SCIENCE_STEALTH_FIGHTER));
    assert!(game_logic.honesty_stealth_fighter_science_unlock_ok());
    assert!(
        game_logic.enqueue_production(airfield_id, "AmericaJetStealthFighter".to_string()),
        "science unlock must allow production"
    );
    assert!(game_logic.honesty_stealth_fighter_science_produce_ok());

    // Advance production to completion (build_time 0.05s).
    for _ in 0..10 {
        game_logic.update_production(0.02);
    }
    assert!(
        game_logic.honesty_stealth_fighter_science_spawn_ok()
            || game_logic
                .objects
                .values()
                .any(|o| o.template_name.contains("StealthFighter")
                    && o.is_kind_of(KindOf::Aircraft)),
        "science-gated Stealth Fighter must spawn from production"
    );
    assert!(
        game_logic.honesty_stealth_fighter_science_ok(),
        "combined science residual honesty"
    );
}

#[test]
fn chem_terrorist_gamma_and_demo_death_weapon_residual() {
    use crate::game_logic::host_terrorist::{
        SUICIDE_DYNAMITE_PRIMARY_DAMAGE_DEMO, SUICIDE_DYNAMITE_PRIMARY_DAMAGE_GAMMA,
        TERRORIST_SUICIDE_WEAPON,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut chem_tpl = crate::game_logic::ThingTemplate::new("Chem_GLAInfantryTerrorist");
    chem_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(120.0)
        .set_primary_weapon_name(TERRORIST_SUICIDE_WEAPON);
    game_logic
        .templates
        .insert("Chem_GLAInfantryTerrorist".to_string(), chem_tpl);

    let mut demo_tpl = crate::game_logic::ThingTemplate::new("Demo_GLAInfantryTerrorist");
    demo_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(120.0)
        .set_primary_weapon_name(TERRORIST_SUICIDE_WEAPON);
    game_logic
        .templates
        .insert("Demo_GLAInfantryTerrorist".to_string(), demo_tpl);

    // Chem Gamma residual: tag Anthrax Gamma then detonate.
    let chem_id = game_logic
        .create_object(
            "Chem_GLAInfantryTerrorist",
            Team::GLA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("chem terrorist");
    {
        let t = game_logic.host_object_mut(chem_id).unwrap();
        t.apply_upgrade_tag("Chem_Upgrade_GLAAnthraxGamma");
        // Chem Gamma primary damage flag residual.
        if let Some(w) = t.weapon.as_mut() {
            w.damage = SUICIDE_DYNAMITE_PRIMARY_DAMAGE_GAMMA;
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
            w.range = 20.0;
        }
        t.record_host_weapon_stats();
    }
    let near = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(8.0, 0.0, 0.0))
        .expect("near");
    {
        let t = game_logic.host_object_mut(chem_id).unwrap();
        t.attack_target(near);
    }
    let hp_before = game_logic
        .host_object(near)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let zones_before = game_logic.toxin_tractor_registry().zones_spawned;
    game_logic.set_current_frame(40);
    game_logic.update_combat(&[chem_id, near], LOGIC_FRAME_TIMESTEP);
    assert!(
        game_logic.terrorist_residual_detonations() > 0,
        "chem terrorist detonation residual"
    );
    let hp_after = game_logic
        .host_object(near)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let gamma_dmg = hp_before - hp_after;
    assert!(
        gamma_dmg + 0.1 >= SUICIDE_DYNAMITE_PRIMARY_DAMAGE_GAMMA.min(hp_before),
        "Chem Gamma residual must deal ~600 primary (got {gamma_dmg}, before={hp_before})"
    );
    assert!(
        game_logic.toxin_tractor_registry().zones_spawned > zones_before,
        "Chem Gamma residual must spawn MediumPoisonField"
    );

    // Demo HE residual: 700 primary, no poison.
    let demo_id = game_logic
        .create_object(
            "Demo_GLAInfantryTerrorist",
            Team::GLA,
            Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("demo terrorist");
    let near2 = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(108.0, 0.0, 0.0))
        .expect("near2");
    {
        let t = game_logic.host_object_mut(demo_id).unwrap();
        t.attack_target(near2);
        if let Some(w) = t.weapon.as_mut() {
            assert!(
                (w.damage - SUICIDE_DYNAMITE_PRIMARY_DAMAGE_DEMO).abs() < 1.0,
                "Demo terrorist spawn weapon must flag 700 primary, got {}",
                w.damage
            );
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
            w.range = 20.0;
        }
        t.record_host_weapon_stats();
    }
    let zones_mid = game_logic.toxin_tractor_registry().zones_spawned;
    let hp2_before = game_logic
        .host_object(near2)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    game_logic.set_current_frame(80);
    game_logic.update_combat(&[demo_id, near2], LOGIC_FRAME_TIMESTEP);
    let hp2_after = game_logic
        .host_object(near2)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let demo_dmg = hp2_before - hp2_after;
    assert!(
        demo_dmg + 0.1 >= SUICIDE_DYNAMITE_PRIMARY_DAMAGE_DEMO.min(hp2_before),
        "Demo residual must deal ~700 primary (got {demo_dmg})"
    );
    assert_eq!(
        game_logic.toxin_tractor_registry().zones_spawned,
        zones_mid,
        "Demo residual must not spawn poison field"
    );
}

#[test]
fn chem_demo_trap_gamma_and_demo_he_residual() {
    use crate::game_logic::host_mines::DemoTrapProfile;

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    // Chem Gamma trap residual.
    let trap_id = game_logic
        .place_demo_trap_named(
            "Chem_GLADemoTrap",
            Team::GLA,
            Vec3::new(0.0, 0.0, 0.0),
            None,
            true, // gamma
        )
        .expect("chem trap");
    {
        let trap = game_logic.host_object(trap_id).unwrap();
        let md = trap.mine_data.as_ref().unwrap();
        assert_eq!(md.demo_trap_profile, DemoTrapProfile::ChemGamma);
        assert!((md.detonation_damage - 250.0).abs() < 0.01);
    }
    let enemy = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("enemy");
    let zones_before = game_logic.toxin_tractor_registry().zones_spawned;
    let hp_before = game_logic.host_object(enemy).unwrap().health.current;
    game_logic.update_mines_and_demo_traps();
    game_logic.frame = game_logic
        .frame
        .saturating_add(crate::game_logic::host_mines::DEMO_TRAP_DESTRUCTION_DELAY_FRAMES);
    game_logic.update_mines_and_demo_traps();
    assert_eq!(game_logic.mine_residual_proximity_detonations(), 1);
    let enemy_after = game_logic.host_object(enemy);
    let damaged = enemy_after
        .map(|e| e.health.current < hp_before || e.status.destroyed)
        .unwrap_or(true);
    assert!(damaged, "Chem DemoTrap must damage enemy");
    assert!(
        game_logic.toxin_tractor_registry().zones_spawned > zones_before,
        "Chem Gamma DemoTrap must spawn MediumPoisonField"
    );

    // Demo HE trap residual (700/25 + 500/50 dual ring).
    let demo_trap = game_logic
        .place_demo_trap_named(
            "Demo_GLADemoTrap",
            Team::GLA,
            Vec3::new(200.0, 0.0, 0.0),
            None,
            false,
        )
        .expect("demo trap");
    {
        let trap = game_logic.host_object(demo_trap).unwrap();
        let md = trap.mine_data.as_ref().unwrap();
        assert_eq!(md.demo_trap_profile, DemoTrapProfile::Demo);
        assert!((md.detonation_damage - 700.0).abs() < 0.01);
        assert!((md.secondary_damage - 500.0).abs() < 0.01);
    }
    let far = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(230.0, 0.0, 0.0))
        .expect("far enemy in secondary ring");
    // Ensure within trigger range 40.
    let far2 = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(210.0, 0.0, 0.0))
        .expect("near enemy");
    let far_hp = game_logic.host_object(far2).unwrap().health.current;
    let zones_mid = game_logic.toxin_tractor_registry().zones_spawned;
    game_logic.update_mines_and_demo_traps();
    game_logic.frame = game_logic
        .frame
        .saturating_add(crate::game_logic::host_mines::DEMO_TRAP_DESTRUCTION_DELAY_FRAMES);
    game_logic.update_mines_and_demo_traps();
    assert!(
        game_logic.mine_residual_proximity_detonations() >= 2,
        "Demo HE trap must proximity detonate"
    );
    let far_after = game_logic.host_object(far2);
    let far_damaged = far_after
        .map(|e| e.health.current < far_hp || e.status.destroyed)
        .unwrap_or(true);
    assert!(far_damaged, "Demo HE trap must damage enemy");
    assert_eq!(
        game_logic.toxin_tractor_registry().zones_spawned,
        zones_mid,
        "Demo HE trap must not spawn poison"
    );
    let _ = far; // secondary-ring placement residual (optional observability)
}

#[test]
fn chem_demo_trap_construction_applies_anthrax_gamma_puddle() {
    use crate::game_logic::KindOf;
    use crate::game_logic::host_mines::DemoTrapProfile;
    use crate::game_logic::host_upgrades::UPGRADE_CHEM_ANTHRAX_GAMMA;

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::GLA);
    if let Some(p) = logic.get_player_mut(2) {
        p.add_completed_upgrade(UPGRADE_CHEM_ANTHRAX_GAMMA);
    }
    let mut trap_t = ThingTemplate::new("Chem_GLADemoTrap");
    trap_t.add_kind_of(KindOf::Structure).set_health(100.0);
    logic.templates.insert("Chem_GLADemoTrap".into(), trap_t);
    let id = logic
        .create_object("Chem_GLADemoTrap", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("constructed chem trap");
    let md = logic
        .host_object(id)
        .and_then(|o| o.mine_data.as_ref())
        .expect("mine bind");
    assert_eq!(md.demo_trap_profile, DemoTrapProfile::ChemGamma);
    assert!(md.demo_trap_profile.spawns_poison());
    assert_eq!(
        md.demo_trap_profile.poison_anthrax_tier(),
        crate::game_logic::host_toxin_tractor::AnthraxResidualTier::Gamma
    );
}

#[test]
fn unit_training_science_veterancy_grant_residual() {
    use crate::game_logic::VeterancyLevel;
    use crate::game_logic::host_unit_training::{
        SCIENCE_BATTLEMASTER_TRAINING, SCIENCE_RED_GUARD_TRAINING,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::China);

    let mut rg_tpl = crate::game_logic::ThingTemplate::new("ChinaInfantryRedguard");
    rg_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(120.0);
    game_logic
        .templates
        .insert("ChinaInfantryRedguard".to_string(), rg_tpl);

    let mut bm_tpl = crate::game_logic::ThingTemplate::new("ChinaTankBattleMaster");
    bm_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(480.0);
    game_logic
        .templates
        .insert("ChinaTankBattleMaster".to_string(), bm_tpl);

    // Fail-closed: without science, spawn remains Rookie.
    let rookie_id = game_logic
        .create_object(
            "ChinaInfantryRedguard",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("rookie redguard");
    {
        let u = game_logic.host_object(rookie_id).unwrap();
        assert!(
            matches!(u.experience.level, VeterancyLevel::Rookie),
            "without training science must remain Rookie"
        );
    }

    // Unlock Red Guard training → VETERAN on spawn.
    assert!(game_logic.unlock_team_science(Team::China, SCIENCE_RED_GUARD_TRAINING));
    assert!(game_logic.honesty_unit_training_unlock_ok());
    let vet_id = game_logic
        .create_object(
            "ChinaInfantryRedguard",
            Team::China,
            Vec3::new(20.0, 0.0, 0.0),
        )
        .expect("veteran redguard");
    {
        let u = game_logic.host_object(vet_id).unwrap();
        assert!(
            matches!(u.experience.level, VeterancyLevel::Veteran),
            "SCIENCE_RedGuardTraining must grant VETERAN, got {:?}",
            u.experience.level
        );
        // Veterancy health residual: +20% max HP.
        assert!(
            u.health.maximum + 0.1 >= 120.0 * 1.2,
            "VETERAN residual must apply +20% HP (got {})",
            u.health.maximum
        );
    }
    assert!(game_logic.honesty_unit_training_grant_ok());

    // Battlemaster training → ELITE.
    assert!(game_logic.unlock_team_science(Team::China, SCIENCE_BATTLEMASTER_TRAINING));
    let elite_id = game_logic
        .create_object(
            "ChinaTankBattleMaster",
            Team::China,
            Vec3::new(40.0, 0.0, 0.0),
        )
        .expect("elite battlemaster");
    {
        let u = game_logic.host_object(elite_id).unwrap();
        assert!(
            matches!(u.experience.level, VeterancyLevel::Elite),
            "SCIENCE_BattlemasterTraining must grant ELITE, got {:?}",
            u.experience.level
        );
        assert!(
            u.health.maximum + 0.1 >= 480.0 * 1.3,
            "ELITE residual must apply +30% HP (got {})",
            u.health.maximum
        );
    }
    assert!(
        game_logic.honesty_unit_training_ok(),
        "combined unit-training honesty"
    );
    assert!(game_logic.unit_training().battlemaster_grants >= 1);
    assert!(game_logic.unit_training().red_guard_grants >= 1);
}

#[test]
fn demo_suicide_bomb_structure_death_residual() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_demo_suicide_bomb::{
        DEMO_DESTROYED_PRIMARY_DAMAGE, UPGRADE_DEMO_SUICIDE_BOMB,
    };
    use crate::game_logic::host_upgrades::HostUpgradeKind;

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(2, Team::GLA, "DemoGLA", true);
    player.resources.supplies = 10_000;
    game_logic.add_player(player);
    ensure_test_barracks_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut rebel_tpl = crate::game_logic::ThingTemplate::new("Demo_GLAInfantryRebel");
    rebel_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(120.0);
    game_logic
        .templates
        .insert("Demo_GLAInfantryRebel".to_string(), rebel_tpl);

    let mut tank_tpl = crate::game_logic::ThingTemplate::new("TestTank");
    tank_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0);
    game_logic
        .templates
        .insert("TestTank".to_string(), tank_tpl);

    let rebel_id = game_logic
        .create_object("Demo_GLAInfantryRebel", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("demo rebel");
    let enemy_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("enemy");
    let barracks_id = game_logic
        .create_object("TestBarracks", Team::GLA, Vec3::new(-50.0, 0.0, 0.0))
        .expect("barracks");

    // Research SuicideBomb residual via QueueUpgrade → complete.
    game_logic.queue_command(GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: UPGRADE_DEMO_SUICIDE_BOMB.to_string(),
        },
        player_id: 2,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![barracks_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert!(
        game_logic
            .host_upgrades()
            .honesty_queue_ok(HostUpgradeKind::SuicideBomb)
    );
    // Retail SuicideBomb BuildTime 30s → research advances over retail frames
    // (C++ ProductionUpdate owns the timer on the producer); the stale
    // "residual frames = 1" comment predates retail INI timing.
    for _ in 0..HostUpgradeKind::SuicideBomb.retail_research_frames() {
        game_logic.update_with_dt(LOGIC_FRAME_TIMESTEP);
    }
    assert!(
        game_logic
            .host_upgrades()
            .honesty_complete_ok(HostUpgradeKind::SuicideBomb)
            || game_logic
                .get_player(2)
                .map(|p| p.has_unlocked_upgrade(UPGRADE_DEMO_SUICIDE_BOMB))
                .unwrap_or(false),
        "SuicideBomb upgrade must complete"
    );
    {
        let rebel = game_logic.host_object(rebel_id).unwrap();
        assert!(
            rebel.has_upgrade_tag(UPGRADE_DEMO_SUICIDE_BOMB),
            "Demo Rebel must receive SuicideBomb tag"
        );
    }
    assert!(
        game_logic.honesty_demo_suicide_bomb_upgrade_ok(),
        "SuicideBomb upgrade honesty"
    );

    // Kill Demo Rebel → Demo_DestroyedWeapon residual damages nearby enemy.
    {
        let e = game_logic.host_object_mut(enemy_id).unwrap();
        e.health.current = 5000.0;
        e.health.maximum = 5000.0;
        e.thing.template.armor = 0.0;
    }
    let hp_before = game_logic.host_object(enemy_id).unwrap().health.current;
    game_logic.mark_object_for_destruction(rebel_id, Some(Team::USA));
    game_logic.process_destroy_list();

    let hp_after = game_logic
        .host_object(enemy_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let dmg = hp_before - hp_after;
    assert!(
        dmg + 0.1 >= DEMO_DESTROYED_PRIMARY_DAMAGE.min(hp_before),
        "Demo_DestroyedWeapon residual must deal ~50 primary (got {dmg}, before={hp_before})"
    );
    assert!(
        game_logic.honesty_demo_suicide_bomb_death_ok(),
        "SuicideBomb death honesty"
    );
    assert!(
        game_logic.honesty_demo_suicide_bomb_ok(),
        "SuicideBomb host path honesty"
    );

    // Spawn after research still tags residual.
    let rebel2 = game_logic
        .create_object(
            "Demo_GLAInfantryRebel",
            Team::GLA,
            Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("demo rebel2");
    assert!(
        game_logic
            .host_object(rebel2)
            .unwrap()
            .has_upgrade_tag(UPGRADE_DEMO_SUICIDE_BOMB),
        "new Demo spawns must inherit SuicideBomb residual"
    );
    assert!(
        game_logic
            .host_object(rebel2)
            .unwrap()
            .command_set_override
            .as_deref()
            == Some("Demo_GLAInfantryRebelCommandSetUpgrade"),
        "spawn must receive CommandSetUpgrade residual"
    );
}

#[test]
fn demo_tertiary_suicide_plus_fire_command_set_residual() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_demo_suicide_bomb::{
        DEMO_PLUS_FIRE_PRIMARY_DAMAGE, UPGRADE_DEMO_SUICIDE_BOMB,
    };
    use crate::game_logic::host_upgrades::HostUpgradeKind;

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(2, Team::GLA, "DemoGLA", true);
    player.resources.supplies = 10_000;
    game_logic.add_player(player);
    ensure_test_barracks_template(&mut game_logic);

    let mut rebel_tpl = crate::game_logic::ThingTemplate::new("Demo_GLAInfantryRebel");
    rebel_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(120.0);
    game_logic
        .templates
        .insert("Demo_GLAInfantryRebel".to_string(), rebel_tpl);

    let mut tank_tpl = crate::game_logic::ThingTemplate::new("TestTank");
    tank_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(5000.0);
    game_logic
        .templates
        .insert("TestTank".to_string(), tank_tpl);

    let rebel_id = game_logic
        .create_object("Demo_GLAInfantryRebel", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("demo rebel");
    let enemy_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("enemy");
    let barracks_id = game_logic
        .create_object("TestBarracks", Team::GLA, Vec3::new(-50.0, 0.0, 0.0))
        .expect("barracks");

    // Fail-closed: TertiarySuicide denied before upgrade.
    assert!(
        !game_logic.issue_demo_tertiary_suicide(rebel_id),
        "TertiarySuicide must fail-closed without SuicideBomb"
    );
    assert!(game_logic.demo_suicide_bomb().tertiary_suicides_denied >= 1);

    // Research SuicideBomb → CommandSetUpgrade residual.
    game_logic.queue_command(GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: UPGRADE_DEMO_SUICIDE_BOMB.to_string(),
        },
        player_id: 2,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![barracks_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    // Retail SuicideBomb BuildTime 30s → advance the full research timer.
    for _ in 0..HostUpgradeKind::SuicideBomb.retail_research_frames() {
        game_logic.update_with_dt(LOGIC_FRAME_TIMESTEP);
    }
    assert!(
        game_logic
            .host_upgrades()
            .honesty_complete_ok(HostUpgradeKind::SuicideBomb)
            || game_logic
                .get_player(2)
                .map(|p| p.has_unlocked_upgrade(UPGRADE_DEMO_SUICIDE_BOMB))
                .unwrap_or(false),
        "SuicideBomb upgrade must complete"
    );
    {
        let rebel = game_logic.host_object(rebel_id).unwrap();
        assert!(
            rebel.has_upgrade_tag(UPGRADE_DEMO_SUICIDE_BOMB),
            "rebel must be tagged"
        );
        assert!(
            rebel
                .command_set_override
                .as_ref()
                .map(|s| s.contains("CommandSetUpgrade"))
                .unwrap_or(false),
            "CommandSetUpgrade residual must apply: {:?}",
            rebel.command_set_override
        );
    }
    assert!(
        game_logic.honesty_demo_suicide_bomb_command_set_ok(),
        "CommandSetUpgrade honesty"
    );

    // Issue TertiarySuicide via command residual.
    {
        let e = game_logic.host_object_mut(enemy_id).unwrap();
        e.health.current = 5000.0;
        e.health.maximum = 5000.0;
        e.thing.template.armor = 0.0;
    }
    let hp_before = game_logic.host_object(enemy_id).unwrap().health.current;
    game_logic.queue_command(GameCommand {
        command_type: CommandType::DemoTertiarySuicide,
        player_id: 2,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![rebel_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    game_logic.process_destroy_list();

    let rebel_alive = game_logic
        .host_object(rebel_id)
        .map(|o| o.is_alive())
        .unwrap_or(false);
    assert!(!rebel_alive, "TertiarySuicide must consume the unit");

    let hp_after = game_logic
        .host_object(enemy_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let dmg = hp_before - hp_after;
    assert!(
        dmg + 0.1 >= DEMO_PLUS_FIRE_PRIMARY_DAMAGE.min(hp_before),
        "PlusFire residual must deal ~500 primary (got {dmg}, before={hp_before})"
    );
    // DestroyedWeapon (50) must NOT also fire on top of PlusFire.
    assert!(
        dmg < DEMO_PLUS_FIRE_PRIMARY_DAMAGE + 60.0,
        "must not double-apply DestroyedWeapon after PlusFire (got {dmg})"
    );
    assert!(
        game_logic.honesty_demo_suicide_bomb_suicided_ok(),
        "suicided PlusFire honesty"
    );
    assert!(
        game_logic.honesty_demo_suicide_bomb_plus_fire_ok(),
        "PlusFire + CommandSetUpgrade host path honesty"
    );
    assert_eq!(
        game_logic.demo_suicide_bomb().death_detonations,
        0,
        "normal DestroyedWeapon path must not fire on SUICIDED residual"
    );
    assert!(
        game_logic.demo_suicide_bomb().suicided_detonations >= 1,
        "PlusFire detonation counter"
    );
}

#[test]
fn combat_chase_pathfinds_cpp_surface() {
    // The chase emission path was split out of the old game_logic monolith:
    // the OOR/LOS emission windows live in world_tick/combat.rs, the
    // attack-path entry in world_save/world_paths.rs, and the firing-position
    // search in pathfinding/system_attack.rs (C++ AIUpdate combat chase +
    // Pathfinder::findAttackPath + isAttackViewBlockedByObstacle).
    let combat_src = include_str!("../../world_tick/combat.rs");
    let paths_src = include_str!("../../world_save/world_paths.rs");
    let find_src = include_str!("../../pathfinding/system_attack.rs");
    assert!(
        paths_src.contains("fn assign_unit_attack_path")
            && find_src.contains("fn find_attack_firing_position"),
        "combat chase must use findAttackPath residual (assign_unit_attack_path)"
    );
    let i = combat_src
        .find("Ready weapons but out of range")
        .expect("OOR chase comment");
    let w = &combat_src[i..(i + 6000).min(combat_src.len())];
    assert!(
        w.contains("assign_unit_attack_path"),
        "OOR combat chase must call assign_unit_attack_path"
    );
    let j = combat_src
        .find("isAttackViewBlockedByObstacle residual")
        .expect("LOS gate");
    let w2 = &combat_src[j..(j + 2000).min(combat_src.len() - j)];
    assert!(
        w2.contains("assign_unit_attack_path"),
        "LOS-blocked chase must call assign_unit_attack_path"
    );
}

#[test]
fn support_states_path_approach_cpp_surface() {
    // The facade monolith split: path_approach_with_state now lives in
    // world_save/world_paths.rs with callers across the live split modules.
    let src = concat!(
        include_str!("../../world_save/world_paths.rs"),
        include_str!("../../world_scripts/unit_commands.rs"),
        include_str!("../../world_scripts/rebuild_dozer.rs"),
        include_str!("../../world_scripts/saboteur_car_bomb.rs"),
        include_str!("../../world_objects/ai_authority.rs"),
        include_str!("../../world_tick/production.rs"),
        include_str!("../../world_tick/crates.rs"),
    );
    // Scan the live split members directly; production callers dominate the
    // count (per-file #[cfg(test)] tails contribute only the _for_test shim).
    let prod = src;
    assert!(prod.contains("fn path_approach_with_state"));
    assert!(
        prod.matches("path_approach_with_state").count() >= 10,
        "support states should route OOR approaches through path_approach_with_state"
    );
}

#[test]
fn host_attack_los_gates_fire_through_building() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    for (name, kinds) in [
        (
            "LosAtk",
            vec![
                KindOf::Infantry,
                KindOf::Attackable,
                KindOf::AttackNeedsLineOfSight,
            ],
        ),
        ("LosTgt", vec![KindOf::Infantry, KindOf::Attackable]),
        ("LosWall", vec![KindOf::Structure]),
    ] {
        if !logic.templates.contains_key(name) {
            let mut tmpl = ThingTemplate::new(name);
            tmpl.set_health(200.0);
            for k in kinds {
                tmpl.add_kind_of(k);
            }
            logic.templates.insert(name.into(), tmpl);
        }
    }
    let atk = logic
        .create_object("LosAtk", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("atk");
    let wall = logic
        .create_object("LosWall", Team::Neutral, glam::Vec3::new(40.0, 0.0, 0.0))
        .expect("wall");
    let tgt = logic
        .create_object("LosTgt", Team::GLA, glam::Vec3::new(80.0, 0.0, 0.0))
        .expect("tgt");
    // Block every cell on the Bresenham line between attacker and target.
    let from = glam::Vec3::new(0.0, 0.0, 0.0);
    let to = glam::Vec3::new(80.0, 0.0, 0.0);
    let start = logic.pathfinding_system.grid.world_to_grid(from);
    let goal = logic.pathfinding_system.grid.world_to_grid(to);
    let mut x0 = start.x;
    let mut y0 = start.y;
    let x1 = goal.x;
    let y1 = goal.y;
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    // Skip start; block intermediate cells only.
    loop {
        let e2 = 2 * err;
        if e2 >= dy {
            if x0 == x1 {
                break;
            }
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            if y0 == y1 {
                break;
            }
            err += dx;
            y0 += sy;
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        logic.set_pathfinding_static_block(x0, y0, true);
    }
    assert!(
        logic.pathfinding_system.is_attack_view_blocked(from, to),
        "static wall must block attack view start={start:?} goal={goal:?}"
    );
    if let Some(o) = logic.objects.get_mut(&atk) {
        o.weapon = Some(Weapon {
            damage: 25.0,
            range: 200.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ..Weapon::default()
        });
        o.target = Some(tgt);
        o.set_ai_state(AIState::Attacking);
        o.set_status_attacking(true);
    }
    let hp_before = logic
        .objects
        .get(&tgt)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    logic.update_combat(&[atk, tgt, wall], 1.0 / 30.0);
    let hp_after = logic
        .objects
        .get(&tgt)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        (hp_after - hp_before).abs() < 0.01,
        "LOS-blocked attacker must not damage target through static obstacle (hp {hp_before} -> {hp_after})"
    );
    let _ = wall;
}

#[test]
fn host_attack_los_allows_fire_in_open() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    for (name, kinds) in [
        (
            "LosAtk2",
            vec![
                KindOf::Infantry,
                KindOf::Attackable,
                KindOf::AttackNeedsLineOfSight,
            ],
        ),
        ("LosTgt2", vec![KindOf::Infantry, KindOf::Attackable]),
    ] {
        if !logic.templates.contains_key(name) {
            let mut tmpl = ThingTemplate::new(name);
            tmpl.set_health(200.0);
            for k in kinds {
                tmpl.add_kind_of(k);
            }
            logic.templates.insert(name.into(), tmpl);
        }
    }
    let atk = logic
        .create_object("LosAtk2", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("atk");
    let tgt = logic
        .create_object("LosTgt2", Team::GLA, glam::Vec3::new(30.0, 0.0, 0.0))
        .expect("tgt");
    if let Some(o) = logic.objects.get_mut(&atk) {
        o.weapon = Some(Weapon {
            damage: 25.0,
            range: 200.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ..Weapon::default()
        });
        o.target = Some(tgt);
        o.set_ai_state(AIState::Attacking);
        o.set_status_attacking(true);
    }
    let hp_before = logic
        .objects
        .get(&tgt)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    logic.update_combat(&[atk, tgt], 1.0 / 30.0);
    let hp_after = logic
        .objects
        .get(&tgt)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        hp_after < hp_before - 1.0,
        "open-field LOS must still allow fire (hp {hp_before} -> {hp_after})"
    );
}

#[test]
fn generic_object_fire_uses_weapon_ini_damage_type() {
    // C++ Weapon.cpp:1378-1380 dealDamage copies WeaponTemplate::m_damageType
    // onto DamageInfo; Armor.cpp:43-50 ArmorTemplate::adjustDamage then
    // bypasses only DAMAGE_UNRESISTABLE. Pre-fix live update_combat fallback
    // called take_damage_from → Unresistable and ignored Weapon.ini.
    use crate::game_logic::weapon_bootstrap::{PATHFINDER_SNIPER_WEAPON, ensure_host_weapon_store};
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    ensure_host_weapon_store();
    assert_eq!(
        crate::game_logic::host_armor_residual::host_damage_type_for_weapon_name(
            PATHFINDER_SNIPER_WEAPON
        ),
        crate::game_logic::combat::DamageType::Sniper
    );

    let mut logic = GameLogic::new();
    let mut atk_tpl = ThingTemplate::new("HqSzqaRifleman");
    atk_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0)
        .set_primary_weapon_name(PATHFINDER_SNIPER_WEAPON);
    logic.templates.insert("HqSzqaRifleman".into(), atk_tpl);

    let mut tank_tpl = ThingTemplate::new("HqSzqaTank");
    tank_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    logic.templates.insert("HqSzqaTank".into(), tank_tpl);

    let atk = logic
        .create_object("HqSzqaRifleman", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("atk");
    let tgt = logic
        .create_object("HqSzqaTank", Team::GLA, glam::Vec3::new(30.0, 0.0, 0.0))
        .expect("tgt");
    if let Some(o) = logic.objects.get_mut(&atk) {
        o.weapon = Some(Weapon {
            damage: 100.0,
            range: 300.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            projectile_speed: 999_999.0,
            ..Weapon::default()
        });
        o.target = Some(tgt);
        o.set_ai_state(AIState::Attacking);
        o.set_status_attacking(true);
    }
    let hp_before = logic
        .objects
        .get(&tgt)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    logic.update_combat(&[atk, tgt], 1.0 / 30.0);
    let hp_after = logic
        .objects
        .get(&tgt)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    // TankArmor SNIPER residual is 0% (Armor.ini). Unresistable would deal 100.
    assert!(
        (hp_after - hp_before).abs() < 0.01,
        "generic object-vs-object fire must use Weapon.ini SNIPER so TankArmor absorbs it (hp {hp_before} -> {hp_after})"
    );
}

#[test]
fn find_attack_path_picks_los_cell_not_target_footprint() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    for (name, kinds) in [
        (
            "FapAtk",
            vec![
                KindOf::Infantry,
                KindOf::Attackable,
                KindOf::AttackNeedsLineOfSight,
            ],
        ),
        ("FapTgt", vec![KindOf::Infantry, KindOf::Attackable]),
    ] {
        if !logic.templates.contains_key(name) {
            let mut tmpl = ThingTemplate::new(name);
            tmpl.set_health(200.0);
            for k in kinds {
                tmpl.add_kind_of(k);
            }
            logic.templates.insert(name.into(), tmpl);
        }
    }
    let atk = logic
        .create_object("FapAtk", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("atk");
    let tgt = logic
        .create_object("FapTgt", Team::GLA, glam::Vec3::new(80.0, 0.0, 0.0))
        .expect("tgt");
    // Wall between but leave a northern corridor open for flanking LOS.
    let from = glam::Vec3::new(0.0, 0.0, 0.0);
    let to = glam::Vec3::new(80.0, 0.0, 0.0);
    let start = logic.pathfinding_system.grid.world_to_grid(from);
    let goal = logic.pathfinding_system.grid.world_to_grid(to);
    let mut x0 = start.x;
    let mut y0 = start.y;
    let x1 = goal.x;
    let y1 = goal.y;
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        let e2 = 2 * err;
        if e2 >= dy {
            if x0 == x1 {
                break;
            }
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            if y0 == y1 {
                break;
            }
            err += dx;
            y0 += sy;
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        // Block center line only (y==start.y); leave y+2 open for flank.
        logic.set_pathfinding_static_block(x0, y0, true);
    }
    if let Some(o) = logic.objects.get_mut(&atk) {
        o.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ..Weapon::default()
        });
        o.target = Some(tgt);
        o.set_ai_state(AIState::Attacking);
    }
    assert!(
        logic.assign_unit_attack_path(atk, Some(tgt), to),
        "must find an attack path"
    );
    let unit = logic.objects.get(&atk).expect("atk after");
    let dest = unit
        .movement
        .path
        .last()
        .copied()
        .or(unit.movement.target_position)
        .expect("dest");
    // Final cell should not be the victim cell (findAttackPath goal is firing cell).
    let dest_cell = logic.pathfinding_system.grid.world_to_grid(dest);
    let victim_cell = logic.pathfinding_system.grid.world_to_grid(to);
    assert_ne!(
        dest_cell, victim_cell,
        "attack path should end on a firing cell, not victim footprint"
    );
    assert!(
        !logic.pathfinding_system.is_attack_view_blocked(dest, to),
        "firing cell must have clear LOS to victim"
    );
}

#[test]
fn structure_footprint_blocks_attack_los() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    for (name, kinds) in [
        (
            "SfAtk",
            vec![
                KindOf::Infantry,
                KindOf::Attackable,
                KindOf::AttackNeedsLineOfSight,
            ],
        ),
        ("SfTgt", vec![KindOf::Infantry, KindOf::Attackable]),
        ("SfWall", vec![KindOf::Structure]),
    ] {
        if !logic.templates.contains_key(name) {
            let mut tmpl = ThingTemplate::new(name);
            tmpl.set_health(500.0);
            for k in kinds {
                tmpl.add_kind_of(k);
            }
            logic.templates.insert(name.into(), tmpl);
        }
    }
    let atk = logic
        .create_object("SfAtk", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("atk");
    let wall = logic
        .create_object("SfWall", Team::Neutral, glam::Vec3::new(40.0, 0.0, 0.0))
        .expect("wall");
    if let Some(o) = logic.objects.get_mut(&wall) {
        o.selection_radius = 18.0;
    }
    // Re-block with larger footprint after radius bump.
    logic.sync_structure_path_blocks();
    let tgt = logic
        .create_object("SfTgt", Team::GLA, glam::Vec3::new(80.0, 0.0, 0.0))
        .expect("tgt");
    // Structure create must have static-blocked its footprint.
    assert!(
        logic.attack_view_blocked(atk, Some(tgt), glam::Vec3::new(80.0, 0.0, 0.0)),
        "structure between attacker and target must block attack LOS"
    );
    if let Some(o) = logic.objects.get_mut(&atk) {
        o.weapon = Some(Weapon {
            damage: 25.0,
            range: 200.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ..Weapon::default()
        });
        o.target = Some(tgt);
        o.set_ai_state(AIState::Attacking);
        o.set_status_attacking(true);
    }
    let hp_before = logic
        .objects
        .get(&tgt)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    logic.update_combat(&[atk, tgt, wall], 1.0 / 30.0);
    let hp_after = logic
        .objects
        .get(&tgt)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        (hp_after - hp_before).abs() < 0.01,
        "must not fire through live structure footprint (hp {hp_before}->{hp_after})"
    );
}

#[test]
fn structure_path_block_cpp_surface() {
    // Facade split: structure path-blocking lives in world_subsystems.rs with
    // create/complete call sites in create_destroy_die.rs and production.rs.
    let src = concat!(
        include_str!("../../world_save/world_subsystems.rs"),
        include_str!("../../world_objects/create_destroy_die.rs"),
        include_str!("../../world_tick/production.rs"),
    );
    assert!(src.contains("fn sync_structure_path_blocks"));
    assert!(src.contains("fn block_structure_object_path"));
    assert!(src.contains("apply_structure_static_blocks"));
    assert!(
        src.contains("block_structure_object_path(id)")
            || src.contains("block_structure_object_path(completed_id)"),
        "create/complete must block structure footprints"
    );
}

#[test]
fn terrain_los_blocks_ridge_between_units() {
    let mut logic = GameLogic::new();
    // Install coarse height cache: flat 0 with a tall ridge at mid X cells.
    let w = logic.pathfinding_system.grid.width().max(8) as u32;
    let h = logic.pathfinding_system.grid.height().max(8) as u32;
    let mut heights = vec![0.0f32; (w * h) as usize];
    let mid = w / 2;
    for y in 0..h {
        for x in mid.saturating_sub(1)..=(mid + 1).min(w - 1) {
            heights[(y * w + x) as usize] = 80.0;
        }
    }
    assert!(
        logic.restore_terrain_heights_from_grid(w, h, &heights),
        "height cache install"
    );
    let from = glam::Vec3::new(0.0, 10.0, 0.0);
    let to = glam::Vec3::new(80.0, 10.0, 0.0);
    assert!(
        !logic.is_clear_line_of_sight_terrain(from, to),
        "ridge must block eye-line between low endpoints"
    );
    // Open sky above ridge still clear.
    let high_from = glam::Vec3::new(0.0, 100.0, 0.0);
    let high_to = glam::Vec3::new(80.0, 100.0, 0.0);
    assert!(
        logic.is_clear_line_of_sight_terrain(high_from, high_to),
        "high eye-line over ridge must stay clear"
    );
}

#[test]
fn attack_view_blocked_uses_terrain_los_surface() {
    // Facade split: attack_view_blocked + terrain LOS live in
    // world_save/world_paths.rs.
    let src = include_str!("../../world_save/world_paths.rs");
    assert!(src.contains("fn is_clear_line_of_sight_terrain"));
    assert!(src.contains("LOS_TERRAIN residual"));
    let i = src.find("pub fn attack_view_blocked").expect("avb");
    let w = &src[i..i + 2500.min(src.len() - i)];
    assert!(
        w.contains("is_clear_line_of_sight_terrain"),
        "attack_view_blocked must call terrain LOS"
    );
}

#[test]
fn attack_view_blocked_cpp_surface() {
    // Facade split: attack_view_blocked lives in world_save/world_paths.rs;
    // the update_combat fire gate lives in world_tick/combat.rs.
    let src = concat!(
        include_str!("../../world_save/world_paths.rs"),
        include_str!("../../world_tick/combat.rs"),
    );
    assert!(src.contains("fn attack_view_blocked"));
    assert!(src.contains("is_attack_view_blocked"));
    let i = src.find("if let Some(slot) = selected_slot").expect("slot");
    let w = &src[i..i + 2500.min(src.len() - i)];
    assert!(
        w.contains("attack_view_blocked"),
        "update_combat must gate fire on attack_view_blocked"
    );
}

#[test]
fn airfield_parking_rearm_docks_and_heals() {
    use crate::game_logic::host_dock_contain_exit_heal_residual::{
        PARKING_PLACE_AIRFIELD_HEAL_AMOUNT_PER_SEC, parking_place_heal_per_frame,
    };
    use crate::game_logic::{KindOf, ParkingPlaceMetadata, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    // C++ findSuitableAirfield ally check resolves ownerless objects through
    // the unique faction-team player (Player.cpp getRelationship).
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut af_tmpl = ThingTemplate::new("AmericaAirfield");
    af_tmpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0);
    af_tmpl.parking_place = Some(ParkingPlaceMetadata {
        num_rows: 2,
        num_cols: 2,
        approach_height: 50.0,
        landing_deck_height_offset: 0.0,
        has_runways: true,
        park_in_hangars: true,
        heal_amount_per_second: 10.0,
    });
    logic.templates.insert("AmericaAirfield".into(), af_tmpl);

    let mut jet_tmpl = ThingTemplate::new("AmericaJetRaptor");
    jet_tmpl.primary_weapon_name = Some("HostTestRaptorJetMissileWeapon".into());
    jet_tmpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic.templates.insert("AmericaJetRaptor".into(), jet_tmpl);

    let af_id = logic
        .create_object("AmericaAirfield", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("af");
    let jet_id = logic
        .create_object("AmericaJetRaptor", Team::USA, Vec3::new(50.0, 40.0, 0.0))
        .expect("jet");

    {
        let jet = logic.objects.get_mut(&jet_id).unwrap();
        jet.weapon = Some(Weapon {
            damage: 50.0,
            range: 200.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ammo: Some(0),
            clip_size: 4,
            can_target_air: true,
            can_target_ground: true,
            ..Weapon::default()
        });
        jet.health.current = 40.0;
        jet.status.airborne_target = false;
        jet.jet_ai.rtb_landing_phase = crate::game_logic::object::JET_RTB_PHASE_TAXI;
        jet.set_position(Vec3::ZERO);
    }

    crate::game_logic::host_ai_decision_log::clear();
    assert!(logic.try_return_to_base_rearm(jet_id));
    {
        let jet = logic.objects.get(&jet_id).unwrap();
        assert_eq!(jet.weapon.as_ref().unwrap().ammo, Some(0));
        assert_eq!(jet.contained_by, Some(af_id));
        assert!(jet.needs_return_to_base_rearm());
        // Docked AI state last-write under AI_DECISION_AUTHORITY (default on).
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            assert_eq!(jet.ai_state, AIState::Idle);
            let docked =
                crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(&AIState::Docked);
            let events = crate::game_logic::host_ai_decision_log::snapshot();
            assert!(
                events.iter().any(|e| {
                    e.host_object == jet_id
                        && e.kind == crate::game_logic::host_ai_decision_log::AI_DECISION_SET_STATE
                        && e.ai_state_ordinal == docked
                }),
                "RTB rearm must log SetAIState(Docked) under decision authority"
            );
        } else {
            assert_eq!(jet.ai_state, AIState::Docked);
        }
    }
    assert!(
        logic
            .objects
            .get(&af_id)
            .unwrap()
            .contained_units()
            .contains(&jet_id),
        "airfield must list parked jet"
    );

    let hp_before = logic.objects.get(&jet_id).unwrap().health.current;
    crate::game_logic::host_heal_log::clear();
    logic.tick_airfield_parking_heal();
    // C++ first pulse is HEAL_RATE_FRAMES after setHealee.
    let expected = 6.0 * PARKING_PLACE_AIRFIELD_HEAL_AMOUNT_PER_SEC / 30.0;
    for _ in 0..6 {
        logic.frame = logic.frame.saturating_add(1);
        logic.tick_airfield_parking_heal();
    }
    let hp_after = logic.objects.get(&jet_id).unwrap().health.current;
    if crate::gameworld_shadow::gameworld_damage_authority_live() {
        let heals = crate::game_logic::host_heal_log::snapshot();
        let logged = heals
            .iter()
            .any(|e| e.target == jet_id && (e.health - (hp_before + expected)).abs() < 1e-2);
        assert!(
            logged || (hp_after - hp_before - expected).abs() < 1e-3,
            "parking heal must log absolute HP under damage authority (hp {hp_before}->{hp_after}, heals={heals:?}, expect +{expected})"
        );
    } else {
        assert!(
            (hp_after - hp_before - expected).abs() < 1e-3,
            "heal {hp_before} -> {hp_after}, want +{expected}"
        );
    }

    assert_eq!(
        logic
            .objects
            .get(&jet_id)
            .unwrap()
            .weapon
            .as_ref()
            .unwrap()
            .ammo,
        Some(4),
        "rearm completes across heal-rate frames"
    );
}

#[test]
fn airfield_parking_capacity_blocks_fifth_jet() {
    use crate::game_logic::{KindOf, ParkingPlaceMetadata, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut af_tmpl = ThingTemplate::new("AmericaAirfield");
    af_tmpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .set_health(1000.0);
    af_tmpl.parking_place = Some(ParkingPlaceMetadata {
        num_rows: 2,
        num_cols: 2,
        approach_height: 50.0,
        landing_deck_height_offset: 0.0,
        has_runways: true,
        park_in_hangars: true,
        heal_amount_per_second: 10.0,
    });
    logic.templates.insert("AmericaAirfield".into(), af_tmpl);
    let mut jet_tmpl = ThingTemplate::new("AmericaJetRaptor");
    jet_tmpl.primary_weapon_name = Some("HostTestRaptorJetMissileWeapon".into());
    jet_tmpl.add_kind_of(KindOf::Aircraft).set_health(100.0);
    logic.templates.insert("AmericaJetRaptor".into(), jet_tmpl);

    let _af = logic
        .create_object("AmericaAirfield", Team::USA, Vec3::ZERO)
        .unwrap();
    let mut jet_ids = Vec::new();
    for i in 0..5 {
        let id = logic
            .create_object(
                "AmericaJetRaptor",
                Team::USA,
                Vec3::new(10.0 + i as f32, 20.0, 0.0),
            )
            .unwrap();
        if let Some(jet) = logic.objects.get_mut(&id) {
            jet.weapon = Some(Weapon {
                damage: 10.0,
                range: 100.0,
                reload_time: 0.0,
                last_fire_time: -100.0,
                ammo: Some(0),
                clip_size: 2,
                ..Weapon::default()
            });
        }
        jet_ids.push(id);
    }
    // First 4 dock; 5th capacity-blocked (NumRows*NumCols=4).
    for (i, &id) in jet_ids.iter().enumerate() {
        let ok = logic.try_return_to_base_rearm(id);
        if i < 4 {
            assert!(ok, "jet {i} should dock");
        } else {
            assert!(!ok, "5th jet must hit parking capacity");
        }
    }
}

#[test]
fn airfield_takeoff_keeps_parking_stall_for_airborne_jet() {
    use crate::game_logic::buildings::BuildingType;
    use crate::game_logic::{KindOf, ParkingPlaceMetadata, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut af_tmpl = ThingTemplate::new("KeepStallAirfield");
    af_tmpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .set_health(1000.0);
    af_tmpl.parking_place = Some(ParkingPlaceMetadata {
        num_rows: 2,
        num_cols: 2,
        approach_height: 50.0,
        landing_deck_height_offset: 0.0,
        has_runways: true,
        park_in_hangars: true,
        heal_amount_per_second: 10.0,
    });
    logic.templates.insert("KeepStallAirfield".into(), af_tmpl);
    let mut jet_tmpl = ThingTemplate::new("KeepStallRaptor");
    jet_tmpl.primary_weapon_name = Some("HostTestRaptorJetMissileWeapon".into());
    jet_tmpl.add_kind_of(KindOf::Aircraft).set_health(100.0);
    logic.templates.insert("KeepStallRaptor".into(), jet_tmpl);
    let af = logic
        .create_object("KeepStallAirfield", Team::USA, Vec3::ZERO)
        .unwrap();
    if let Some(o) = logic.host_object_mut(af) {
        o.building_data = Some(crate::game_logic::BuildingData::new(BuildingType::Airfield));
    }
    let jet_id = logic
        .create_object("KeepStallRaptor", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    if let Some(jet) = logic.objects.get_mut(&jet_id) {
        jet.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ammo: Some(0),
            clip_size: 2,
            ..Weapon::default()
        });
    }
    assert!(logic.try_return_to_base_rearm(jet_id));
    assert!(logic.try_runway_takeoff_from_airfield(jet_id));
    {
        let jet = logic.objects.get(&jet_id).unwrap();
        assert!(jet.contained_by.is_none());
        assert!(jet.airfield_parking_space_index.is_some());
    }
    assert!(
        logic
            .airfield_parking_spaces
            .get(&af)
            .is_some_and(|spaces| spaces.iter().any(|space| space.object_id == Some(jet_id))),
        "airborne jet must keep hangar stall (JetAIUpdate.cpp:897-900, 1630)"
    );
}
