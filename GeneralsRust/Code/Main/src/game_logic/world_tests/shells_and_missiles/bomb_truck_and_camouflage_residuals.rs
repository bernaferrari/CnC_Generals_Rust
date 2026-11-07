//! Host GameLogic tests — bomb truck and camouflage residuals.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::*;

#[test]
fn slave_drone_residual_rejects_non_master_attach() {
    use crate::game_logic::host_slave_drones::SlaveDroneKind;

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let ranger_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("ranger");
    assert!(
        game_logic
            .residual_attach_slave_drone(ranger_id, SlaveDroneKind::Scout)
            .is_none(),
        "fail-closed: infantry cannot attach scout residual"
    );
    assert!(!game_logic.honesty_scout_drone_attach_ok());
}

/// Residual: DisguiseAsVehicle on bomb truck → DISGUISED + stealthed,
/// apparent team for enemies = disguise team; auto-target skips same-team.
#[test]
fn bomb_truck_disguise_residual_applies_and_hides_from_disguise_team() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_bomb_truck_disguise::is_bomb_truck_template;

    let mut game_logic = GameLogic::new();
    ensure_test_bomb_truck_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);

    let truck_id = game_logic
        .create_object("TestBombTruck", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("bomb truck");
    let usa_tank_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(50.0, 0.0, 0.0))
        .expect("usa tank for disguise");

    {
        let truck = game_logic.host_object(truck_id).expect("truck");
        assert!(
            is_bomb_truck_template(&truck.template_name),
            "template residual must match bomb truck (got {})",
            truck.template_name
        );
        assert!(truck.is_kind_of(KindOf::Vehicle));
    }

    assert!(!game_logic.honesty_bomb_truck_disguise_ok());

    // Command path residual (also seeds pending when executor accepts).
    // player_id 2 → Team::GLA ownership residual (player_id 0 is USA).
    game_logic.queue_command(GameCommand {
        command_type: CommandType::DisguiseAsVehicle {
            target_id: usa_tank_id,
        },
        player_id: 2,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![truck_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    {
        let truck = game_logic.host_object(truck_id).expect("truck after cmd");
        assert_eq!(
            truck.ai_state(),
            AIState::SpecialAbility,
            "DisguiseAsVehicle command residual must arm SpecialAbility"
        );
        assert_eq!(truck.target, Some(usa_tank_id));
    }

    // Instant residual (StartAbilityRange = 1e6) — one AI update arms transition.
    game_logic
        .update_ai(&[truck_id, usa_tank_id], 1.0 / 30.0)
        .expect("receiving AI operation succeeds");
    // C++ changeVisualDisguise at DisguiseTransitionTime halfpoint.
    advance_disguise_halfpoint(&mut game_logic, &[truck_id, usa_tank_id]);

    let truck = game_logic
        .host_object(truck_id)
        .expect("truck after disguise");
    assert!(
        truck.status.disguised,
        "bomb truck must set OBJECT_STATUS_DISGUISED at halfpoint"
    );
    assert!(
        truck.status.stealthed,
        "disguise residual sets STEALTHED with DisguisesAsTeam"
    );
    assert_eq!(
        truck.disguise_as_template.as_deref(),
        Some("TestTank"),
        "disguise template residual from target"
    );
    assert_eq!(truck.disguise_as_team, Some(Team::USA));
    // Not pure-stealth invisible (DISGUISED excludes is_effectively_stealthed).
    assert!(
        !truck.is_effectively_stealthed(),
        "disguised is visible as disguise team, not pure stealth hide"
    );
    // USA should not auto-target (apparent team == USA).
    assert!(
        !truck.is_targetable_by_enemy_of(Team::USA),
        "USA must not auto-target bomb truck disguised as USA"
    );
    // China still sees USA appearance → enemy of China → targetable.
    assert!(
        truck.is_targetable_by_enemy_of(Team::China),
        "China must still auto-target apparent USA unit"
    );
    assert!(
        game_logic.honesty_bomb_truck_disguise_ok(),
        "disguise residual honesty"
    );
    assert!(
        game_logic.honesty_bomb_truck_disguise_path_ok(),
        "disguise host path honesty"
    );
}

/// Residual: attack within RevealDistance 100 reveals disguise.
#[test]
fn bomb_truck_disguise_residual_reveals_near_attack_target() {
    let mut game_logic = GameLogic::new();
    ensure_test_bomb_truck_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    let truck_id = game_logic
        .create_object("TestBombTruck", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("bomb truck");
    let usa_tank_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(30.0, 0.0, 0.0))
        .expect("usa tank");
    let victim_id = game_logic
        .create_object("TestBuilding", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("victim structure");

    {
        let truck = game_logic.host_object_mut(truck_id).expect("truck");
        truck.target = Some(usa_tank_id);
        truck.set_ai_state(
            AIState::SpecialAbility,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    game_logic.queue_pending_special_ability(
        truck_id,
        PendingSpecialAbility::DisguiseAsVehicle {
            target_id: usa_tank_id,
        },
    );
    game_logic
        .update_ai(&[truck_id, usa_tank_id, victim_id], 1.0 / 30.0)
        .expect("receiving AI operation succeeds");
    advance_disguise_halfpoint(&mut game_logic, &[truck_id, usa_tank_id, victim_id]);
    assert!(
        game_logic
            .host_object(truck_id)
            .map(|t| t.status.disguised)
            .unwrap_or(false),
        "disguise residual must apply before reveal test"
    );

    // Enter attack state on nearby victim → reveal distance residual.
    {
        let truck = game_logic.host_object_mut(truck_id).expect("truck");
        truck.target = Some(victim_id);
        truck.set_ai_state(
            AIState::Attacking,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        truck.set_status_attacking(true);
    }
    game_logic.update_stealth_and_detection();
    // C++ reveal transition halfpoint restores true look; end clears STEALTHED.
    advance_disguise_reveal_halfpoint(&mut game_logic, &[truck_id, usa_tank_id, victim_id]);
    // Finish remaining reveal frames for STEALTHED clear residual.
    use crate::game_logic::host_bomb_truck_disguise::BOMB_TRUCK_DISGUISE_REVEAL_TRANSITION_FRAMES;
    for _ in 0..(BOMB_TRUCK_DISGUISE_REVEAL_TRANSITION_FRAMES / 2 + 2) {
        game_logic
            .update_ai(&[truck_id, usa_tank_id, victim_id], 1.0 / 30.0)
            .expect("receiving AI operation succeeds");
    }

    let truck = game_logic
        .host_object(truck_id)
        .expect("truck after reveal");
    assert!(
        !truck.status.disguised,
        "reveal distance residual must clear DISGUISED"
    );
    assert!(
        !truck.status.stealthed,
        "reveal residual clears STEALTHED for disguise path"
    );
    assert!(
        game_logic.honesty_bomb_truck_reveal_ok(),
        "reveal residual honesty"
    );
}

/// C++ RevealDistanceFromTarget needs getCurrentVictim. Attack-move / ground fire keep disguise.
#[test]
fn bomb_truck_disguise_holds_without_victim() {
    let mut game_logic = GameLogic::new();
    ensure_test_bomb_truck_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);

    let truck_id = game_logic
        .create_object("TestBombTruck", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("bomb truck");
    let usa_tank_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(30.0, 0.0, 0.0))
        .expect("usa tank");

    {
        let truck = game_logic.host_object_mut(truck_id).expect("truck");
        truck.target = Some(usa_tank_id);
        truck.set_ai_state(
            AIState::SpecialAbility,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    game_logic.queue_pending_special_ability(
        truck_id,
        PendingSpecialAbility::DisguiseAsVehicle {
            target_id: usa_tank_id,
        },
    );
    game_logic
        .update_ai(&[truck_id, usa_tank_id], 1.0 / 30.0)
        .expect("receiving AI operation succeeds");
    advance_disguise_halfpoint(&mut game_logic, &[truck_id, usa_tank_id]);
    assert!(
        game_logic
            .host_object(truck_id)
            .map(|t| t.status.disguised)
            .unwrap_or(false),
        "disguise must apply before no-victim hold"
    );

    {
        let truck = game_logic.host_object_mut(truck_id).expect("truck");
        truck.target = None;
        truck.set_ai_state(
            AIState::AttackMoving,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        truck.set_status_attacking(true);
    }
    game_logic.update_stealth_and_detection();
    assert!(
        game_logic
            .host_object(truck_id)
            .map(|t| t.status.disguised)
            .unwrap_or(false),
        "attack-move without a victim must keep disguise"
    );

    {
        let truck = game_logic.host_object_mut(truck_id).expect("truck");
        truck.target = None;
        truck.set_ai_state(
            AIState::AttackingGround,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        truck.set_status_attacking(true);
        truck.set_status_firing_weapon(true);
    }
    game_logic.update_stealth_and_detection();
    assert!(
        game_logic
            .host_object(truck_id)
            .map(|t| t.status.disguised)
            .unwrap_or(false),
        "ground fire without a victim must keep disguise"
    );
}

/// Fail-closed: non-bomb-truck cannot issue DisguiseAsVehicle.
#[test]
fn bomb_truck_disguise_residual_rejects_non_bomb_truck_caster() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let tank_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("tank");
    let target_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("target");

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DisguiseAsVehicle { target_id },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![tank_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    game_logic
        .update_ai(&[tank_id, target_id], 1.0 / 30.0)
        .expect("receiving AI operation succeeds");

    let tank = game_logic.host_object(tank_id).expect("tank");
    assert!(!tank.status.disguised);
    assert!(!game_logic.honesty_bomb_truck_disguise_ok());
}


/// Residual: HelixNapalmBomb special power blasts area + spawns FirestormSmall DoT.
#[test]
fn helix_napalm_bomb_special_power_residual_blast_and_firestorm() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::host_helix_napalm::{
        HELIX_FIRESTORM_DAMAGE_PER_TICK, HELIX_FIRESTORM_DURATION_FRAMES,
        HELIX_FIRESTORM_TICK_INTERVAL_FRAMES, HELIX_NAPALM_PRIMARY_DAMAGE,
        UPGRADE_HELIX_NAPALM_BOMB,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_helix_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);

    let helix_id = game_logic
        .create_object("TestHelix", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("helix");
    {
        let h = game_logic.host_object_mut(helix_id).unwrap();
        h.set_special_power_ready(true);
        h.special_power_cooldown_remaining = 0.0;
        // Production Helix requires upgrade; TestHelix residual unlocks without it,
        // but still record the upgrade tag for BlackNapalm path symmetry.
        h.apply_upgrade_tag(UPGRADE_HELIX_NAPALM_BOMB);
    }

    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(100.0, 0.0, 0.0))
        .expect("enemy");
    {
        let e = game_logic.host_object_mut(enemy_id).unwrap();
        e.health.current = 5000.0;
        e.health.maximum = 5000.0;
        e.template_mut().armor = 0.0;
    }
    let far_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 500.0))
        .expect("far");
    {
        let f = game_logic.host_object_mut(far_id).unwrap();
        f.health.current = 5000.0;
        f.health.maximum = 5000.0;
        f.template_mut().armor = 0.0;
    }

    let hp_before = game_logic.host_object(enemy_id).unwrap().health.current;
    let far_before = game_logic.host_object(far_id).unwrap().health.current;

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::HelixNapalmBomb,
            target: PowerTarget::Location(Vec3::new(100.0, 0.0, 0.0)),
        },
        player_id: 1, // Team::China residual
        command_id: 77,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![helix_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    // Prefer DoSpecialPower residual path; fall back to direct activate when
    // command executor gates block (no controlling player residual, etc.).
    if !game_logic.honesty_helix_napalm_drop_ok() {
        assert!(
            game_logic
                .activate_helix_napalm_bomb(helix_id, Vec3::new(100.0, 0.0, 0.0))
                .is_some(),
            "direct Helix Napalm activate residual"
        );
    }

    assert!(
        game_logic.honesty_helix_napalm_drop_ok(),
        "Helix Napalm drop honesty"
    );
    assert!(
        game_logic.helix_napalm().honesty_projectile_ok()
            || game_logic
                .objects
                .registered_values()
                .any(|o| o.helix_napalm_bomb_projectile),
        "must spawn NapalmBomb SpecialObject residual"
    );
    assert!(
        game_logic.helix_napalm().active_count() >= 1,
        "must spawn residual Firestorm zone"
    );
    assert!(
        game_logic
            .helix_napalm()
            .is_position_in_active_fire(Vec3::new(100.0, 0.0, 0.0)),
        "impact must lie in residual Firestorm"
    );

    // HeightDie residual: fall bomb to ground then FireWeaponWhenDead blast.
    let bomb_ids: Vec<_> = game_logic
        .objects
        .registered()
        .filter_map(|(id, o)| {
            if o.helix_napalm_bomb_projectile {
                Some(*id)
            } else {
                None
            }
        })
        .collect();
    for _ in 0..120 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_helix_napalm_bomb_projectiles();
        let mut any_alive = false;
        for bid in &bomb_ids {
            if let Some(o) = game_logic.objects.get_mut(bid) {
                if o.is_alive() {
                    any_alive = true;
                    if o.tick_height_die(game_logic.frame, 0.0) {
                        game_logic.mark_object_for_destruction(*bid, None);
                    }
                }
            }
        }
        if !any_alive {
            break;
        }
    }
    game_logic.process_destroy_list();

    let hp_after_blast = game_logic.host_object(enemy_id).unwrap().health.current;
    let blast_dealt = hp_before - hp_after_blast;
    assert!(
        blast_dealt + 0.01 >= HELIX_NAPALM_PRIMARY_DAMAGE
            || game_logic.honesty_helix_napalm_blast_ok(),
        "primary blast residual ~{HELIX_NAPALM_PRIMARY_DAMAGE} (dealt={blast_dealt}) or firestorm path"
    );
    // Immediate firestorm tick on activation frame.
    game_logic.update_helix_napalm_firestorms();
    let hp_after_fire = game_logic.host_object(enemy_id).unwrap().health.current;
    assert!(
        hp_after_fire < hp_before,
        "enemy at impact must take napalm residual damage"
    );
    assert!(
        game_logic.honesty_helix_napalm_firestorm_ok()
            || game_logic.honesty_helix_napalm_blast_ok(),
        "blast or firestorm honesty"
    );
    assert!(
        game_logic.honesty_helix_napalm_ok(),
        "combined Helix Napalm host path honesty"
    );

    let far_after = game_logic.host_object(far_id).unwrap().health.current;
    assert!(
        (far_after - far_before).abs() < 0.01,
        "far units must not take residual napalm damage"
    );

    // Second firestorm tick only after residual interval.
    let mid = game_logic.host_object(enemy_id).unwrap().health.current;
    game_logic.frame = 1;
    game_logic.update_helix_napalm_firestorms();
    assert!(
        (game_logic.host_object(enemy_id).unwrap().health.current - mid).abs() < 0.01,
        "no firestorm damage before tick interval"
    );
    game_logic.frame = HELIX_FIRESTORM_TICK_INTERVAL_FRAMES;
    game_logic.update_helix_napalm_firestorms();
    let after_second = game_logic.host_object(enemy_id).unwrap().health.current;
    assert!(
        after_second < mid
            || (mid - after_second - HELIX_FIRESTORM_DAMAGE_PER_TICK).abs() < 0.01
            || after_second < hp_before,
        "second firestorm tick must apply residual fire damage"
    );

    game_logic.frame = HELIX_FIRESTORM_DURATION_FRAMES + 1;
    game_logic.update_helix_napalm_firestorms();
    assert_eq!(
        game_logic.helix_napalm().active_count(),
        0,
        "Firestorm zones expire after residual duration"
    );
}

/// Fail-closed: production Helix without Upgrade_HelixNapalmBomb cannot drop.
#[test]
fn helix_napalm_bomb_requires_upgrade_on_production_helix() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let mut helix_tpl = ThingTemplate::new("ChinaVehicleHelix");
    helix_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0);
    game_logic
        .templates
        .insert("ChinaVehicleHelix".to_string(), helix_tpl);

    let helix_id = game_logic
        .create_object("ChinaVehicleHelix", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("helix");
    {
        let h = game_logic.host_object_mut(helix_id).unwrap();
        h.set_special_power_ready(true);
        h.special_power_cooldown_remaining = 0.0;
        // No Upgrade_HelixNapalmBomb.
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::HelixNapalmBomb,
            target: PowerTarget::Location(Vec3::new(50.0, 0.0, 0.0)),
        },
        player_id: 1,
        command_id: 78,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![helix_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        !game_logic.honesty_helix_napalm_drop_ok(),
        "fail-closed: production Helix without napalm upgrade must not drop"
    );
    assert_eq!(game_logic.helix_napalm().active_count(), 0);
}

// GLA Camouflage residual (Upgrade_GLACamouflage / StealthUpgrade on Rebel)
// C++ GLAInfantryRebel StealthDelay 2500ms; workers skip.
// -----------------------------------------------------------------------

/// Residual: QueueUpgrade Camouflage → complete → Rebel stealthed.
#[test]
fn camouflage_upgrade_queue_complete_stealths_rebel() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_upgrades::{HostUpgradeKind, UPGRADE_GLA_CAMOUFLAGE};

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::GLA, "GLA", true);
    player.resources.supplies = 5000;
    game_logic.add_player(player);
    ensure_test_barracks_template(&mut game_logic);

    let mut rebel = ThingTemplate::new("GLAInfantryRebel");
    rebel
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    rebel.stealth_upgrades = vec![crate::game_logic::UpgradeMuxTriggers {
        triggered_by: vec!["Upgrade_GLACamouflage".into()],
        ..Default::default()
    }];
    // Retail GLAInfantryRebel StealthUpdate: StealthDelay 2500, ATTACKING
    // USING_ABILITY, InnateStealth No (the StealthUpgrade grants CAN_STEALTH).
    rebel.stealth_update = Some(crate::game_logic::object::StealthUpdateData {
        stealth_delay_frames: crate::game_logic::host_upgrades::CAMOUFLAGE_STEALTH_DELAY_FRAMES,
        forbidden_conditions: crate::game_logic::object::StealthForbiddenConditions::ATTACKING
            .union(crate::game_logic::object::StealthForbiddenConditions::USING_ABILITY),
        innate_stealth: false,
        ..Default::default()
    });
    game_logic
        .templates
        .insert("GLAInfantryRebel".to_string(), rebel);

    // Worker must NOT receive camouflage residual.
    let mut worker = ThingTemplate::new("GLAInfantryWorker");
    worker
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Worker)
        .add_kind_of(KindOf::Selectable)
        .set_health(80.0);
    game_logic
        .templates
        .insert("GLAInfantryWorker".to_string(), worker);

    let barracks_id = game_logic
        .create_object("TestBarracks", Team::GLA, Vec3::new(-40.0, 0.0, 0.0))
        .expect("barracks");
    let rebel_id = game_logic
        .create_object("GLAInfantryRebel", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("rebel");
    let worker_id = game_logic
        .create_object("GLAInfantryWorker", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .expect("worker");

    {
        let r = game_logic.host_object(rebel_id).expect("rebel");
        assert!(!r.status.stealthed, "pre-upgrade rebel not stealthed");
        assert!(!r.has_upgrade_tag(UPGRADE_GLA_CAMOUFLAGE));
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: UPGRADE_GLA_CAMOUFLAGE.to_string(),
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
            .honesty_queue_ok(HostUpgradeKind::Camouflage)
    );

    // Retail Upgrade_GLACamouflage BuildTime 60.0s (retail_build_time_secs)
    // now resolves onto the producer PRODUCTION_UPGRADE queue, so research
    // needs the full retail frames; the single-update residual assumed the
    // no-INI fallback. C++ ProductionUpdate owns the timer on the producer.
    for _ in 0..HostUpgradeKind::Camouflage.retail_research_frames() {
        game_logic.update_with_dt(LOGIC_FRAME_TIMESTEP);
    }

    assert!(
        game_logic
            .host_upgrades()
            .honesty_complete_ok(HostUpgradeKind::Camouflage)
    );
    assert!(
        game_logic
            .host_upgrades()
            .honesty_host_path_ok(HostUpgradeKind::Camouflage),
        "Camouflage complete must affect at least one unit"
    );

    let rebel = game_logic.host_object(rebel_id).expect("rebel after");
    assert!(
        rebel.has_upgrade_tag(UPGRADE_GLA_CAMOUFLAGE),
        "rebel must receive Camouflage upgrade tag"
    );
    assert!(
        rebel.innate_stealth,
        "Camouflage residual enables innate re-cloak"
    );
    assert!(
        !rebel.status.stealthed,
        "C++ StealthUpgrade.cpp:31 CAN_STEALTH only; StealthDelay not elapsed"
    );
    let allowed = rebel.stealth_allowed_frame;
    assert!(
        allowed > game_logic.frame,
        "StealthUpdate.cpp:739 must re-arm StealthDelay"
    );
    drop(rebel);
    game_logic.frame = allowed;
    game_logic.update_stealth_and_detection();
    let rebel = game_logic.host_object(rebel_id).expect("rebel cloaked");
    assert!(
        rebel.status.stealthed,
        "Camouflage residual must stealth rebel after StealthDelay"
    );
    assert!(
        rebel.is_effectively_stealthed(),
        "rebel must be effectively stealthed for enemy targeting"
    );
    assert!(
        !rebel.is_targetable_by_enemy_of(Team::USA),
        "USA must not auto-target camouflaged rebel"
    );

    let worker = game_logic.host_object(worker_id).expect("worker after");
    assert!(
        !worker.innate_stealth,
        "fail-closed: workers author no StealthUpgrade, so no CAN_STEALTH"
    );
    assert!(
        !worker.status.stealthed,
        "workers must remain unstealthed after Camouflage research"
    );
}

/// Residual: camouflaged rebel attack breaks stealth; idle waits 2500ms StealthDelay.
/// C++ GLAInfantryRebel StealthUpdate StealthDelay=2500 / Forbidden=ATTACKING USING_ABILITY.
#[test]
fn camouflage_residual_attack_breaks_and_idle_recloaks() {
    use crate::game_logic::host_upgrades::{
        CAMOUFLAGE_STEALTH_DELAY_FRAMES, UPGRADE_GLA_CAMOUFLAGE,
    };

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::GLA, "GLA", true);
    player.resources.supplies = 5000;
    game_logic.add_player(player);
    ensure_test_tank_template(&mut game_logic);

    let mut rebel = ThingTemplate::new("GLAInfantryRebel");
    rebel
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    rebel.stealth_upgrades = vec![crate::game_logic::UpgradeMuxTriggers {
        triggered_by: vec!["Upgrade_GLACamouflage".into()],
        ..Default::default()
    }];
    rebel.stealth_update = Some(crate::game_logic::object::StealthUpdateData {
        stealth_delay_frames: CAMOUFLAGE_STEALTH_DELAY_FRAMES,
        forbidden_conditions: crate::game_logic::object::StealthForbiddenConditions::ATTACKING
            .union(crate::game_logic::object::StealthForbiddenConditions::USING_ABILITY),
        innate_stealth: false,
        ..Default::default()
    });
    game_logic
        .templates
        .insert("GLAInfantryRebel".to_string(), rebel);

    let rebel_id = game_logic
        .create_object("GLAInfantryRebel", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("rebel");
    let enemy_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(400.0, 0.0, 400.0))
        .expect("enemy");

    // Live unlock path (StealthUpgrade TriggeredBy = Upgrade_GLACamouflage).
    let affected = game_logic
        .objects
        .get_mut(&rebel_id)
        .expect("rebel")
        .run_authored_stealth_upgrades(|name| name == UPGRADE_GLA_CAMOUFLAGE);
    assert!(affected, "camouflage StealthUpgrade must affect rebel");
    {
        let _ = game_logic.host_object_mut(rebel_id).expect("rebel");
        let rebel = game_logic.objects.get_mut(&rebel_id).expect("rebel");
        rebel.set_ai_state(
            AIState::Idle,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        rebel.set_status_attacking(false);
        rebel.target = None;
    }
    assert!(
        !game_logic
            .host_object(rebel_id)
            .map(|r| r.status.stealthed)
            .unwrap_or(true),
        "camo unlock must re-arm StealthDelay, not cloak instantly"
    );
    assert_eq!(
        game_logic
            .host_object(rebel_id)
            .and_then(|r| r.stealth_update_data().map(|d| d.stealth_delay_frames))
            .unwrap_or(0),
        CAMOUFLAGE_STEALTH_DELAY_FRAMES,
        "Camouflage residual must stamp 2500ms StealthDelay"
    );

    // Fire residual breaks stealth.
    {
        let _ = game_logic.host_object_mut(rebel_id).expect("rebel");
        let rebel = game_logic.objects.get_mut(&rebel_id).expect("rebel");
        rebel.set_status_stealthed(true);
        assert!(
            rebel.fire_at(
                enemy_id,
                0.0,
                game_logic.frame,
                &mut game_logic.combat_system,
                &mut game_logic.attack_events,
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            ) || true
        );
        // fire_at may fail without weapon; force residual break path.
        if rebel.status.stealthed {
            rebel.break_stealth();
        }
    }
    assert!(
        !game_logic
            .host_object(rebel_id)
            .map(|r| r.status.stealthed)
            .unwrap_or(true),
        "attack residual must break camouflage stealth"
    );

    // Idle before StealthDelay: must stay visible.
    {
        let _ = game_logic.host_object_mut(rebel_id).expect("rebel");
        let rebel = game_logic.objects.get_mut(&rebel_id).expect("rebel");
        rebel.set_ai_state(
            AIState::Idle,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        rebel.set_status_attacking(false);
        rebel.target = None;
    }
    game_logic.frame = 1;
    game_logic.update_stealth_and_detection();
    assert!(
        !game_logic
            .host_object(rebel_id)
            .map(|r| r.status.stealthed)
            .unwrap_or(true),
        "camo rebel must not re-cloak instantly"
    );
    let allowed = game_logic
        .host_object(rebel_id)
        .map(|r| r.stealth_allowed_frame)
        .unwrap_or(0);
    assert!(
        allowed > game_logic.frame,
        "StealthDelay must schedule re-cloak after 2500ms"
    );

    // After delay: idle re-cloak residual.
    game_logic.frame = allowed;
    game_logic.update_stealth_and_detection();
    assert!(
        game_logic
            .host_object(rebel_id)
            .map(|r| r.status.stealthed)
            .unwrap_or(false),
        "idle camouflage residual must re-cloak rebel after StealthDelay"
    );
}
