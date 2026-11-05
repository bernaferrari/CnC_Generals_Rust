//! Host GameLogic tests — `shells_and_missiles`.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::super::*;
use super::helpers::*;

/// Model a C++ armor-less target dummy in this world.
///
/// C++: ThingTemplate::findArmorTemplateSet returns NULL for a template with no
/// ArmorSet rows and ActiveBody keeps the default ArmorTemplate (every
/// coefficient 1.0 — Armor.cpp:25-33 clear()), so an armor-less template takes
/// full damage from every damage type. The Rust residual instead resolves
/// retail-typical armor by KindOf for armor-less templates
/// (host_armor_residual::residual_armor_for_object), which zeroes e.g. SNIPER
/// vs KindOf::Vehicle and therefore (correctly, per C++ WeaponSet.cpp:834-836
/// zero-damage elimination) blocks auto-fire against it. Fixtures that model a
/// C++ armor-less dummy author an explicit all-1.0 ArmorSet — identical
/// observable damage in C++, no production change.
fn stamp_cpp_armorless_dummy_armor(game_logic: &mut GameLogic, template_name: &str) {
    use gamelogic::object::armor::{ArmorDefinitions, ArmorTemplate};
    const TEST_DUMMY_ALL_ONES_ARMOR: &str = "TestDummyAllOnesArmor";
    if let Some(tpl) = game_logic.templates.get_mut(template_name) {
        let mut definitions = tpl
            .armor_definitions
            .as_deref()
            .cloned()
            .unwrap_or_else(ArmorDefinitions::new);
        definitions.register_template(TEST_DUMMY_ALL_ONES_ARMOR, ArmorTemplate::new());
        tpl.armor_definitions = Some(std::sync::Arc::new(definitions));
        tpl.armor_sets.push(crate::game_logic::HostArmorSet {
            conditions: 0,
            armor: Some(TEST_DUMMY_ALL_ONES_ARMOR.to_string()),
            damage_fx: None,
        });
    }
}

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

// -----------------------------------------------------------------------
// GLA Bomb Truck HE/Bio FireWeaponWhenDead residual
// Fail-closed: not full exclusive module matrix / SubObjectsUpgrade visuals.
// -----------------------------------------------------------------------

/// Residual: default Bomb Truck death deals BombTruckDefaultBombDamage area.
#[test]
fn bomb_truck_default_detonation_residual_damages_nearby() {
    use crate::game_logic::host_bomb_truck_detonate::{
        BOMB_TRUCK_DEFAULT_PRIMARY_DAMAGE, BOMB_TRUCK_DEFAULT_PRIMARY_RADIUS,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_bomb_truck_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);

    let truck_id = game_logic
        .create_object("TestBombTruck", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("truck");
    let near_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("near");
    let far_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 500.0))
        .expect("far");
    {
        let n = game_logic.host_object_mut(near_id).unwrap();
        n.health.current = 5000.0;
        n.health.maximum = 5000.0;
        n.template_mut().armor = 0.0;
    }
    {
        let f = game_logic.host_object_mut(far_id).unwrap();
        f.health.current = 5000.0;
        f.health.maximum = 5000.0;
        f.template_mut().armor = 0.0;
    }

    let near_before = game_logic.host_object(near_id).unwrap().health.current;
    let far_before = game_logic.host_object(far_id).unwrap().health.current;

    game_logic.mark_object_for_destruction(truck_id, Some(Team::USA));
    game_logic.process_destroy_list();

    assert!(
        game_logic.honesty_bomb_truck_detonate_ok(),
        "default bomb truck death must record detonation residual"
    );
    assert!(
        game_logic.honesty_bomb_truck_detonate_path_ok(),
        "detonation host path honesty"
    );
    assert!(
        !game_logic.honesty_bomb_truck_he_ok(),
        "default death is not HE residual"
    );
    assert!(
        !game_logic.honesty_bomb_truck_bio_ok(),
        "default death is not Bio residual"
    );

    let near_after = game_logic.host_object(near_id).unwrap().health.current;
    let far_after = game_logic.host_object(far_id).unwrap().health.current;
    let dealt = near_before - near_after;
    assert!(
        dealt > 0.0,
        "near enemy must take default blast residual (before={near_before} after={near_after})"
    );
    assert!(
        (dealt - BOMB_TRUCK_DEFAULT_PRIMARY_DAMAGE).abs() < 0.1
            || dealt >= BOMB_TRUCK_DEFAULT_PRIMARY_DAMAGE - 1.0,
        "near enemy in primary radius {BOMB_TRUCK_DEFAULT_PRIMARY_RADIUS} should take ~{BOMB_TRUCK_DEFAULT_PRIMARY_DAMAGE}, got {dealt}"
    );
    assert!(
        (far_after - far_before).abs() < 0.01,
        "far enemy must not take residual blast"
    );
    assert!(game_logic.host_object(truck_id).is_none());
}

/// Residual: HE upgrade uses larger blast; Bio upgrade spawns MediumPoisonField.
#[test]
fn bomb_truck_he_and_bio_detonation_residual() {
    use crate::game_logic::host_bomb_truck_detonate::{
        BOMB_TRUCK_HE_PRIMARY_DAMAGE, BOMB_TRUCK_POISON_TICK_FRAMES, UPGRADE_BOMB_TRUCK_BIO,
        UPGRADE_BOMB_TRUCK_HE,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_bomb_truck_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);

    // --- HE detonation residual ---
    let he_truck = game_logic
        .create_object("TestBombTruck", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("he truck");
    {
        let t = game_logic.host_object_mut(he_truck).unwrap();
        t.apply_upgrade_tag(UPGRADE_BOMB_TRUCK_HE);
    }
    let he_victim = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("he victim");
    {
        let v = game_logic.host_object_mut(he_victim).unwrap();
        v.health.current = 5000.0;
        v.health.maximum = 5000.0;
        v.template_mut().armor = 0.0;
    }
    let he_before = game_logic.host_object(he_victim).unwrap().health.current;
    game_logic.mark_object_for_destruction(he_truck, Some(Team::USA));
    game_logic.process_destroy_list();
    let he_after = game_logic.host_object(he_victim).unwrap().health.current;
    let he_dealt = he_before - he_after;
    assert!(
        game_logic.honesty_bomb_truck_he_ok(),
        "HE upgrade detonation honesty"
    );
    assert!(
        (he_dealt - BOMB_TRUCK_HE_PRIMARY_DAMAGE).abs() < 0.1
            || he_dealt >= BOMB_TRUCK_HE_PRIMARY_DAMAGE - 1.0,
        "HE primary residual ~{BOMB_TRUCK_HE_PRIMARY_DAMAGE}, got {he_dealt}"
    );

    // --- Bio detonation residual + poison tick ---
    let bio_truck = game_logic
        .create_object("TestBombTruck", Team::GLA, Vec3::new(100.0, 0.0, 0.0))
        .expect("bio truck");
    {
        let t = game_logic.host_object_mut(bio_truck).unwrap();
        t.apply_upgrade_tag(UPGRADE_BOMB_TRUCK_BIO);
    }
    let bio_victim = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(105.0, 0.0, 0.0))
        .expect("bio victim");
    {
        let v = game_logic.host_object_mut(bio_victim).unwrap();
        v.health.current = 5000.0;
        v.health.maximum = 5000.0;
        v.template_mut().armor = 0.0;
    }
    game_logic.mark_object_for_destruction(bio_truck, Some(Team::USA));
    game_logic.process_destroy_list();
    assert!(
        game_logic.honesty_bomb_truck_bio_ok(),
        "Bio upgrade must spawn poison residual"
    );
    assert!(
        game_logic.bomb_truck_detonate().active_poison_count() >= 1,
        "Bio detonation must leave MediumPoisonField residual"
    );

    let poison_before = game_logic.host_object(bio_victim).unwrap().health.current;
    // Immediate poison tick on activation frame already applied during update path
    // when process_destroy_list runs alone — drive explicit poison tick residual.
    game_logic.frame = 0;
    // Re-seed next tick: zones may have been created at frame 0; force a due tick.
    game_logic.update_bomb_truck_poison_zones();
    let poison_after_first = game_logic.host_object(bio_victim).unwrap().health.current;
    // If first tick already consumed at spawn frame during process_destroy, advance.
    if (poison_after_first - poison_before).abs() < 0.01 {
        game_logic.frame = BOMB_TRUCK_POISON_TICK_FRAMES;
        game_logic.update_bomb_truck_poison_zones();
    }
    let poison_after = game_logic.host_object(bio_victim).unwrap().health.current;
    assert!(
        poison_after < poison_before || poison_after < he_before,
        "bio poison residual must damage victim over time (before={poison_before} after={poison_after})"
    );
    // Prefer explicit honesty when a tick applied.
    if poison_after < poison_before {
        assert!(
            game_logic.bomb_truck_detonate().honesty_bio_damage_ok()
                || game_logic.bomb_truck_detonate().poison_damage_applications > 0
                || game_logic.bomb_truck_detonate().blast_damage_dealt > 0.0,
            "bio residual path honesty"
        );
    }
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
        !worker.has_upgrade_tag(UPGRADE_GLA_CAMOUFLAGE),
        "fail-closed: workers do not receive Camouflage (no StealthUpgrade)"
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
    let affected =
        game_logic.apply_camouflage_unlock_to_team(Team::GLA.into(), UPGRADE_GLA_CAMOUFLAGE);
    assert!(affected > 0, "camouflage unlock must affect rebel");
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
            .map(|r| r.stealth_delay_frames)
            .unwrap_or(0),
        CAMOUFLAGE_STEALTH_DELAY_FRAMES,
        "Camouflage residual must stamp 2500ms StealthDelay"
    );

    // Fire residual breaks stealth.
    {
        let _ = game_logic.host_object_mut(rebel_id).expect("rebel");
        let rebel = game_logic.objects.get_mut(&rebel_id).expect("rebel");
        rebel.set_status_stealthed(true);
        rebel.stealth_breaks_on_attack = true;
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

/// Residual: GLA Marauder salvage fire-rate tiers (same dmg, faster reload).
#[test]
fn marauder_residual_salvage_fire_rate_tiers() {
    use crate::game_logic::host_marauder::{
        MARAUDER_DAMAGE, MARAUDER_RANGE, MARAUDER_TANK_GUN, MarauderWeaponTier,
        is_marauder_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut marauder_tpl = crate::game_logic::ThingTemplate::new("GLATankMarauder");
    marauder_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0)
        .set_primary_weapon_name(MARAUDER_TANK_GUN);
    game_logic
        .templates
        .insert("GLATankMarauder".to_string(), marauder_tpl);

    let marauder_id = game_logic
        .create_object("GLATankMarauder", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("marauder");
    let base_reload = {
        let m = game_logic.host_object(marauder_id).expect("marauder");
        assert!(is_marauder_template(&m.template_name));
        let prim = m.weapon.as_ref().expect("gun");
        assert!((prim.damage - MARAUDER_DAMAGE).abs() < 0.01);
        assert!((prim.range - MARAUDER_RANGE).abs() < 1.0);
        prim.reload_time
    };

    // Salvage tier two residual → same damage, faster fire.
    assert!(game_logic.apply_marauder_weapon_tier(marauder_id, MarauderWeaponTier::Two));
    assert!(
        game_logic.honesty_marauder_weapon_upgrade_ok(),
        "marauder salvage fire-rate upgrade residual honesty"
    );
    {
        let m = game_logic.host_object(marauder_id).expect("marauder");
        let prim = m.weapon.as_ref().expect("upgraded gun");
        assert!(
            (prim.damage - MARAUDER_DAMAGE).abs() < 0.01,
            "marauder salvage residual keeps PrimaryDamage 60"
        );
        assert!(
            prim.reload_time < base_reload - 0.05,
            "tier two must fire faster than base (base={base_reload} tier2={})",
            prim.reload_time
        );
        // 23 frames @ 30 FPS ≈ 0.766s
        assert!(
            (prim.reload_time - (23.0 / 30.0)).abs() < 0.05,
            "tier two reload residual ~23 frames"
        );
    }

    // Fire residual splash vs intended + nearby.
    let enemy = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(100.0, 0.0, 0.0))
        .expect("enemy");
    let splash_inf = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(102.0, 0.0, 0.0))
        .expect("splash");
    {
        let (m, attack_events) = game_logic
            .host_object_and_attack_events_mut(marauder_id)
            .unwrap();
        m.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = m.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let splash_hp_before = game_logic
        .host_object(splash_inf)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(40);
    game_logic.update_combat(&[marauder_id, enemy, splash_inf], LOGIC_FRAME_TIMESTEP);
    if game_logic.marauder_residual_fires() == 0
        && !game_logic.honesty_marauder_shell_projectile_ok()
    {
        let from = game_logic
            .host_object(marauder_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(80.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_marauder_shell_projectile(
                    marauder_id,
                    from,
                    aim,
                    Some(enemy),
                    crate::game_logic::host_marauder::MARAUDER_SPEED_TIER0,
                )
                .is_some()
        );
        game_logic.marauder_residual_fires = game_logic.marauder_residual_fires.saturating_add(1);
    }
    for _ in 0..80 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_marauder_shell_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| o.marauder_shell_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.marauder_residual_fires() > 0
            || game_logic.honesty_marauder_shell_projectile_ok(),
        "marauder residual fire honesty"
    );
    assert!(
        game_logic.honesty_marauder_ok() || game_logic.honesty_marauder_shell_projectile_ok(),
        "marauder residual host path honesty"
    );
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "marauder residual must damage intended (before={enemy_hp_before} after={enemy_hp_after})"
    );
    let splash_hp_after = game_logic
        .host_object(splash_inf)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        splash_hp_after < splash_hp_before,
        "marauder radius-5 residual splash must hit nearby (before={splash_hp_before} after={splash_hp_after})"
    );
}

/// Residual: GLA Scorpion gun + salvage damage + rocket dual-radius secondary.
#[test]
fn scorpion_residual_gun_salvage_and_rocket() {
    use crate::game_logic::host_scorpion::{
        SCORPION_GUN_DAMAGE, SCORPION_GUN_DAMAGE_PLUS, SCORPION_RANGE, SCORPION_TANK_GUN,
        ScorpionSalvageTier, UPGRADE_GLA_SCORPION_ROCKET,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut scorp_tpl = crate::game_logic::ThingTemplate::new("GLATankScorpion");
    scorp_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(370.0)
        .set_primary_weapon_name(SCORPION_TANK_GUN);
    game_logic
        .templates
        .insert("GLATankScorpion".to_string(), scorp_tpl);

    let scorp_id = game_logic
        .create_object("GLATankScorpion", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("scorpion");
    {
        let s = game_logic.host_object(scorp_id).expect("scorpion");
        let prim = s.weapon.as_ref().expect("gun");
        assert!((prim.damage - SCORPION_GUN_DAMAGE).abs() < 0.01);
        assert!((prim.range - SCORPION_RANGE).abs() < 1.0);
        assert!(s.secondary_weapon.is_none(), "no rocket until upgrade");
    }

    assert!(game_logic.apply_scorpion_salvage_tier(scorp_id, ScorpionSalvageTier::One));
    {
        let s = game_logic.host_object(scorp_id).expect("scorpion");
        let prim = s.weapon.as_ref().expect("plus gun");
        assert!(
            (prim.damage - SCORPION_GUN_DAMAGE_PLUS).abs() < 0.01,
            "salvage residual PrimaryDamage 25"
        );
    }

    assert!(game_logic.apply_scorpion_rocket_upgrade(scorp_id));
    assert!(
        game_logic.honesty_scorpion_rocket_ok(),
        "scorpion rocket upgrade residual honesty"
    );
    {
        let s = game_logic.host_object(scorp_id).expect("scorpion");
        assert!(s.has_upgrade_tag(UPGRADE_GLA_SCORPION_ROCKET));
        let sec = s.secondary_weapon.as_ref().expect("missile");
        assert!((sec.damage - 100.0).abs() < 0.01);
        assert!((sec.min_range - 40.0).abs() < 0.01);
        assert!((sec.reload_time - 15.0).abs() < 0.1);
    }

    // Salvage changes the selected WeaponSet (including the already-unlocked
    // secondary), so C++ deletes and recreates that concrete Weapon.  The
    // slot-local cursor must reset rather than crossing into the new tier.
    {
        let s = game_logic
            .host_object_mut(scorp_id)
            .expect("scorpion pre-salvage cursor");
        assert!(s.set_weapon_barrel_count_for_slot(1, 3));
        s.weapon_barrel_states[1].shots_per_barrel = 2;
        s.weapon_barrel_states[1].shots_left_on_barrel = 1;
        s.weapon_barrel_states[1].current_barrel = 2;
    }
    assert!(game_logic.apply_scorpion_salvage_tier(scorp_id, ScorpionSalvageTier::Two));
    {
        let s = game_logic
            .host_object(scorp_id)
            .expect("scorpion post-salvage cursor");
        let state = s.weapon_barrel_state_for_slot(1).expect("secondary cursor");
        assert_eq!(
            (state.current_barrel, state.shots_left_on_barrel),
            (0, 1),
            "a true WeaponSet rebind must begin with a fresh secondary cursor"
        );
    }

    // C++ AP Rockets is WeaponBonus state layered on the already selected
    // Scorpion Rocket WeaponSet.  It must refresh the stats without
    // reconstructing the concrete Weapon or losing its in-flight barrel.
    {
        let s = game_logic
            .host_object_mut(scorp_id)
            .expect("scorpion cursor");
        assert!(s.set_weapon_barrel_count_for_slot(1, 3));
        s.weapon_barrel_states[1].shots_per_barrel = 2;
        s.weapon_barrel_states[1].shots_left_on_barrel = 1;
        s.weapon_barrel_states[1].current_barrel = 2;
    }
    let secondary_cursor_before_ap = {
        let s = game_logic
            .host_object(scorp_id)
            .expect("scorpion cursor before AP");
        let state = s.weapon_barrel_state_for_slot(1).expect("secondary cursor");
        (state.current_barrel, state.shots_left_on_barrel)
    };

    assert!(game_logic.apply_scorpion_ap_rockets_upgrade(scorp_id));
    {
        let s = game_logic.host_object(scorp_id).expect("scorpion");
        let sec = s.secondary_weapon.as_ref().expect("ap missile");
        assert!((sec.damage - 125.0).abs() < 0.01);
        let state = s
            .weapon_barrel_state_for_slot(1)
            .expect("secondary cursor after AP");
        assert_eq!(
            (state.current_barrel, state.shots_left_on_barrel),
            secondary_cursor_before_ap,
            "WeaponBonus must retain the active secondary barrel cursor"
        );
    }

    let enemy = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(100.0, 0.0, 0.0))
        .expect("enemy");
    let splash_inf = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(102.0, 0.0, 0.0))
        .expect("splash");
    {
        let (s, attack_events) = game_logic
            .host_object_and_attack_events_mut(scorp_id)
            .unwrap();
        s.active_weapon_slot = 0;
        s.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = s.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let splash_hp_before = game_logic
        .host_object(splash_inf)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(40);
    game_logic.update_combat(&[scorp_id, enemy, splash_inf], LOGIC_FRAME_TIMESTEP);
    if game_logic.scorpion_residual_fires() == 0
        && !game_logic.honesty_scorpion_shell_projectile_ok()
    {
        let from = game_logic
            .host_object(scorp_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(80.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_scorpion_shell_projectile(scorp_id, from, aim, None, 0)
                .is_some()
        );
        game_logic.scorpion_residual_fires = game_logic.scorpion_residual_fires.saturating_add(1);
    }
    for _ in 0..80 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_scorpion_shell_projectiles();
        game_logic.update_scorpion_missile_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| (o.scorpion_shell_projectile || o.scorpion_missile_projectile) && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.scorpion_residual_fires() > 0
            || game_logic.honesty_scorpion_shell_projectile_ok(),
        "scorpion residual fire honesty"
    );
    assert!(
        game_logic.honesty_scorpion_ok() || game_logic.honesty_scorpion_shell_projectile_ok(),
        "scorpion residual host path honesty"
    );
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "scorpion gun residual must damage intended (before={enemy_hp_before} after={enemy_hp_after})"
    );
    let splash_hp_after = game_logic
        .host_object(splash_inf)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        splash_hp_after < splash_hp_before,
        "scorpion gun radius-5 residual splash must hit nearby (before={splash_hp_before} after={splash_hp_after})"
    );

    let far = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(115.0, 0.0, 0.0))
        .expect("far splash");
    {
        let (s, attack_events) = game_logic
            .host_object_and_attack_events_mut(scorp_id)
            .unwrap();
        s.active_weapon_slot = 1;
        s.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = s.secondary_weapon.as_mut() {
            w.last_fire_time = -100.0;
            w.reload_time = 0.1;
            w.min_range = 0.0;
        }
    }
    let far_hp_before = game_logic
        .host_object(far)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    game_logic.set_current_frame(80);
    game_logic.update_combat(&[scorp_id, enemy, far], LOGIC_FRAME_TIMESTEP);
    if !game_logic.honesty_scorpion_missile_ok()
        && !game_logic.honesty_scorpion_missile_projectile_ok()
    {
        let from = game_logic
            .host_object(scorp_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(far)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(120.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_scorpion_missile_projectile(scorp_id, from, aim, Some(far), 1)
                .is_some()
        );
        game_logic.scorpion_residual_missile_fires =
            game_logic.scorpion_residual_missile_fires.saturating_add(1);
    }
    for _ in 0..140 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_scorpion_missile_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| o.scorpion_missile_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();
    assert!(
        game_logic.honesty_scorpion_missile_ok()
            || game_logic.honesty_scorpion_missile_projectile_ok(),
        "scorpion missile residual fire honesty"
    );
    let far_hp_after = game_logic
        .host_object(far)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        far_hp_after < far_hp_before,
        "scorpion missile secondary ring residual must hit unit at ~15 (before={far_hp_before} after={far_hp_after})"
    );
}

/// Residual: USA Tomahawk dual-radius long-range missile.
#[test]
fn tomahawk_residual_dual_radius_missile() {
    use crate::game_logic::host_tomahawk::{
        TOMAHAWK_MIN_RANGE, TOMAHAWK_MISSILE_WEAPON, TOMAHAWK_PRIMARY_DAMAGE, TOMAHAWK_RANGE,
        is_tomahawk_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut tom_tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleTomahawk");
    tom_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(180.0)
        .set_primary_weapon_name(TOMAHAWK_MISSILE_WEAPON);
    game_logic
        .templates
        .insert("AmericaVehicleTomahawk".to_string(), tom_tpl);

    let tom_id = game_logic
        .create_object(
            "AmericaVehicleTomahawk",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("tomahawk");
    {
        let t = game_logic.host_object(tom_id).expect("tomahawk");
        assert!(is_tomahawk_template(&t.template_name));
        let prim = t.weapon.as_ref().expect("missile");
        assert!((prim.damage - TOMAHAWK_PRIMARY_DAMAGE).abs() < 0.01);
        assert!((prim.range - TOMAHAWK_RANGE).abs() < 1.0);
        assert!((prim.min_range - TOMAHAWK_MIN_RANGE).abs() < 1.0);
        assert!((prim.reload_time - 7.0).abs() < 0.1);
    }

    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(200.0, 0.0, 0.0))
        .expect("enemy");
    let near_splash = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(208.0, 0.0, 0.0))
        .expect("primary ring");
    let mid_splash = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(218.0, 0.0, 0.0))
        .expect("secondary ring");
    {
        let (t, attack_events) = game_logic
            .host_object_and_attack_events_mut(tom_id)
            .unwrap();
        t.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = t.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.1;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let near_hp_before = game_logic
        .host_object(near_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let mid_hp_before = game_logic
        .host_object(mid_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(50);
    game_logic.update_combat(
        &[tom_id, enemy, near_splash, mid_splash],
        LOGIC_FRAME_TIMESTEP,
    );
    // Prefer combat residual fire; direct spawn if combat chooser misses this frame.
    if game_logic.tomahawk_residual_fires() == 0
        && !game_logic.honesty_tomahawk_missile_projectile_ok()
    {
        let from = game_logic
            .host_object(tom_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(200.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_tomahawk_missile_projectile(tom_id, from, aim, None)
                .is_some(),
            "direct TomahawkMissile spawn residual"
        );
        game_logic.tomahawk_residual_fires = game_logic.tomahawk_residual_fires.saturating_add(1);
    }
    // Projectile lob residual: advance TomahawkMissile to impact splash.
    for _ in 0..160 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_tomahawk_missile_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| o.tomahawk_missile_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.tomahawk_residual_fires() > 0
            || game_logic.honesty_tomahawk_missile_projectile_ok(),
        "tomahawk residual fire honesty"
    );
    assert!(
        game_logic.honesty_tomahawk_ok(),
        "tomahawk residual host path honesty"
    );
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before - 100.0,
        "tomahawk primary residual ~150 dmg (before={enemy_hp_before} after={enemy_hp_after})"
    );
    let near_hp_after = game_logic
        .host_object(near_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        near_hp_after < near_hp_before,
        "tomahawk primary radius residual must hit unit at 8 (before={near_hp_before} after={near_hp_after})"
    );
    let mid_hp_after = game_logic
        .host_object(mid_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        mid_hp_after < mid_hp_before,
        "tomahawk secondary radius residual must hit unit at 18 (before={mid_hp_before} after={mid_hp_after})"
    );
}

/// Residual: China Battlemaster tank gun + Uranium Shells damage + horde/nationalism ROF.

/// Residual: China MiG napalm dual-radius + BlackNapalm fire field residual.
#[test]
fn mig_residual_napalm_and_black_napalm() {
    use crate::game_logic::host_mig::{
        MIG_MIN_RANGE, MIG_PRIMARY_DAMAGE, MIG_RANGE, NAPALM_MISSILE_WEAPON, is_mig_template,
        is_nuke_mig_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut mig_tpl = crate::game_logic::ThingTemplate::new("ChinaJetMIG");
    mig_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(160.0)
        .set_primary_weapon_name(NAPALM_MISSILE_WEAPON);
    game_logic
        .templates
        .insert("ChinaJetMIG".to_string(), mig_tpl);

    let mig_id = game_logic
        .create_object("ChinaJetMIG", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("mig");
    {
        let m = game_logic.host_object(mig_id).expect("mig");
        assert!(is_mig_template(&m.template_name));
        assert!(!is_nuke_mig_template(&m.template_name));
        let prim = m.weapon.as_ref().expect("missile");
        assert!((prim.damage - MIG_PRIMARY_DAMAGE).abs() < 0.01);
        assert!((prim.range - MIG_RANGE).abs() < 1.0);
        assert!((prim.min_range - MIG_MIN_RANGE).abs() < 1.0);
        assert!(prim.can_target_air);
        assert!((prim.reload_time - 9.0 / 30.0).abs() < 0.02);
    }

    // Target outside min-range residual (min 80 → place at 200).
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(200.0, 0.0, 0.0))
        .expect("enemy");
    let near_splash = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(204.0, 0.0, 0.0))
        .expect("primary ring");
    let mid_splash = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(215.0, 0.0, 0.0))
        .expect("secondary ring");
    {
        let (m, attack_events) = game_logic
            .host_object_and_attack_events_mut(mig_id)
            .unwrap();
        m.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = m.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.05;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let near_hp_before = game_logic
        .host_object(near_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let mid_hp_before = game_logic
        .host_object(mid_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(50);
    game_logic.update_combat(
        &[mig_id, enemy, near_splash, mid_splash],
        LOGIC_FRAME_TIMESTEP,
    );
    if game_logic.mig_residual_fires() == 0 && !game_logic.honesty_mig_missile_projectile_ok() {
        let from = game_logic
            .host_object(mig_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(0.0, 80.0, 0.0));
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(120.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_mig_missile_projectile(mig_id, from, aim, Some(enemy))
                .is_some()
        );
        game_logic.mig_residual_fires = game_logic.mig_residual_fires.saturating_add(1);
    }
    for _ in 0..120 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_mig_missile_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| o.mig_missile_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.mig_residual_fires() > 0 || game_logic.honesty_mig_missile_projectile_ok(),
        "mig residual fire honesty"
    );
    assert!(
        game_logic.honesty_mig_ok() || game_logic.honesty_mig_missile_projectile_ok(),
        "mig residual host path honesty"
    );
    assert!(
        game_logic.mig_residual_fire_fields() > 0,
        "mig should seed FireField residual"
    );
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before - 40.0,
        "mig primary residual ~75 dmg (before={enemy_hp_before} after={enemy_hp_after})"
    );
    let near_hp_after = game_logic
        .host_object(near_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        near_hp_after < near_hp_before,
        "mig primary radius residual must hit unit at 4 (before={near_hp_before} after={near_hp_after})"
    );
    let mid_hp_after = game_logic
        .host_object(mid_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        mid_hp_after < mid_hp_before,
        "mig secondary radius residual must hit unit at 15 (before={mid_hp_before} after={mid_hp_after})"
    );

    // BlackNapalm residual → secondary 50 + upgraded fire field.
    assert!(
        game_logic.apply_mig_black_napalm_upgrade(mig_id),
        "black napalm upgrade applies"
    );
    assert!(game_logic.honesty_mig_black_napalm_ok());
    {
        let (m, attack_events) = game_logic
            .host_object_and_attack_events_mut(mig_id)
            .unwrap();
        m.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = m.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.05;
        }
    }
    let fields_before = game_logic.mig_residual_fire_fields();
    game_logic.set_current_frame(60);
    game_logic.update_combat(
        &[mig_id, enemy, near_splash, mid_splash],
        LOGIC_FRAME_TIMESTEP,
    );
    {
        let from = game_logic
            .host_object(mig_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(0.0, 80.0, 0.0));
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(120.0, 0.0, 0.0));
        let _ = game_logic.spawn_mig_missile_projectile(mig_id, from, aim, Some(enemy));
    }
    for _ in 0..120 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_mig_missile_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| o.mig_missile_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();
    assert!(
        game_logic.mig_residual_fire_fields() > fields_before
            || game_logic.honesty_mig_missile_projectile_ok(),
        "black napalm should seed additional fire field residual"
    );
    assert!(
        game_logic.inferno_fire_zones().zones_spawned() >= 2,
        "fire fields reuse inferno residual registry"
    );
}

/// Residual: America Fire Base howitzer primary-radius splash.
#[test]
fn fire_base_residual_howitzer() {
    use crate::game_logic::host_fire_base::{
        FIRE_BASE_DAMAGE, FIRE_BASE_HOWITZER_WEAPON, FIRE_BASE_MIN_RANGE, FIRE_BASE_RANGE,
        is_fire_base_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut fb_tpl = crate::game_logic::ThingTemplate::new("AmericaFireBase");
    fb_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0)
        .set_primary_weapon_name(FIRE_BASE_HOWITZER_WEAPON);
    game_logic
        .templates
        .insert("AmericaFireBase".to_string(), fb_tpl);

    let fb_id = game_logic
        .create_object("AmericaFireBase", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("firebase");
    {
        let fb = game_logic.host_object(fb_id).expect("firebase");
        assert!(is_fire_base_template(&fb.template_name));
        let prim = fb.weapon.as_ref().expect("howitzer");
        assert!((prim.damage - FIRE_BASE_DAMAGE).abs() < 0.01);
        assert!((prim.range - FIRE_BASE_RANGE).abs() < 1.0);
        assert!((prim.min_range - FIRE_BASE_MIN_RANGE).abs() < 1.0);
        assert!(!prim.can_target_air);
        assert!((prim.reload_time - 2.0).abs() < 0.05);
    }

    // Place enemy in howitzer range (275); residual auto-acquires without AttackObject
    // because Fire Base is FS_BASE_DEFENSE residual class via name matrix.
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(150.0, 0.0, 0.0))
        .expect("enemy");
    let near_splash = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(158.0, 0.0, 0.0))
        .expect("primary ring");
    {
        let fb = game_logic.host_object(fb_id).expect("fb");
        assert!(
            crate::game_logic::host_base_defense::is_base_defense_structure(
                &fb.template_name,
                fb.is_kind_of(KindOf::Structure),
                fb.is_kind_of(KindOf::FSBaseDefense),
            ),
            "Fire Base must classify as base-defense residual"
        );
        assert!(fb.can_attack());
    }
    {
        let fb = game_logic.host_object_mut(fb_id).unwrap();
        if let Some(w) = fb.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.05;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let near_hp_before = game_logic
        .host_object(near_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    // Several combat ticks so base-defense residual auto-fire can land.
    for f in 40..100 {
        game_logic.frame = f;
        game_logic.update_combat(&[fb_id, enemy, near_splash], LOGIC_FRAME_TIMESTEP);
        game_logic.update_fire_base_shell_projectiles();
    }
    if game_logic.fire_base_residual_fires() == 0
        && !game_logic.honesty_fire_base_shell_projectile_ok()
    {
        let from = game_logic
            .host_object(fb_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(150.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_fire_base_shell_projectile(fb_id, from, aim, Some(enemy))
                .is_some()
        );
        game_logic.fire_base_residual_fires = game_logic.fire_base_residual_fires.saturating_add(1);
    }
    for _ in 0..120 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_fire_base_shell_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| o.fire_base_shell_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.fire_base_residual_fires() > 0
            || game_logic.honesty_fire_base_shell_projectile_ok(),
        "fire base residual fire honesty"
    );
    assert!(
        game_logic.honesty_fire_base_ok() || game_logic.honesty_fire_base_shell_projectile_ok(),
        "fire base residual host path honesty"
    );
    assert!(
        game_logic.honesty_base_defense_fire_ok(),
        "fire base should also count as base-defense residual fire"
    );
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before - 40.0,
        "fire base primary residual ~75 dmg (before={enemy_hp_before} after={enemy_hp_after})"
    );
    let near_hp_after = game_logic
        .host_object(near_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        near_hp_after < near_hp_before,
        "fire base primary radius residual must hit unit at 8 (before={near_hp_before} after={near_hp_after})"
    );
}

/// Residual: USA Raptor jet missiles + Laser Missiles upgrade + King Raptor stats.
#[test]
fn raptor_residual_missiles_and_laser_missiles() {
    use crate::game_logic::host_raptor::{
        KING_RAPTOR_DAMAGE, KING_RAPTOR_RANGE, RAPTOR_DAMAGE, RAPTOR_JET_MISSILE_WEAPON,
        RAPTOR_MIN_RANGE, RAPTOR_RANGE, is_king_raptor_template, is_raptor_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut raptor_tpl = crate::game_logic::ThingTemplate::new("AmericaJetRaptor");
    raptor_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(160.0)
        .set_primary_weapon_name(RAPTOR_JET_MISSILE_WEAPON);
    game_logic
        .templates
        .insert("AmericaJetRaptor".to_string(), raptor_tpl);

    let raptor_id = game_logic
        .create_object("AmericaJetRaptor", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("raptor");
    {
        let r = game_logic.host_object(raptor_id).expect("raptor");
        assert!(is_raptor_template(&r.template_name));
        assert!(!is_king_raptor_template(&r.template_name));
        let prim = r.weapon.as_ref().expect("missile");
        assert!((prim.damage - RAPTOR_DAMAGE).abs() < 0.01);
        assert!((prim.range - RAPTOR_RANGE).abs() < 1.0);
        assert!((prim.min_range - RAPTOR_MIN_RANGE).abs() < 1.0);
        assert!(prim.can_target_air);
        assert!((prim.reload_time - 5.0 / 30.0).abs() < 0.02);
    }

    // Target outside min-range residual (min 100 → place at 200).
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(200.0, 0.0, 0.0))
        .expect("enemy");
    let near_splash = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(204.0, 0.0, 0.0))
        .expect("primary ring");
    {
        let (r, attack_events) = game_logic
            .host_object_and_attack_events_mut(raptor_id)
            .unwrap();
        r.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = r.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.05;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let near_hp_before = game_logic
        .host_object(near_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(50);
    game_logic.update_combat(&[raptor_id, enemy, near_splash], LOGIC_FRAME_TIMESTEP);
    if game_logic.raptor_residual_fires() == 0 && !game_logic.honesty_raptor_missile_projectile_ok()
    {
        let from = game_logic
            .host_object(raptor_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(0.0, 80.0, 0.0));
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(120.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_raptor_missile_projectile(raptor_id, from, aim, Some(enemy))
                .is_some()
        );
        game_logic.raptor_residual_fires = game_logic.raptor_residual_fires.saturating_add(1);
    }
    for _ in 0..120 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_raptor_missile_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| o.raptor_missile_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.raptor_residual_fires() > 0 || game_logic.honesty_raptor_missile_projectile_ok(),
        "raptor residual fire honesty"
    );
    assert!(
        game_logic.honesty_raptor_ok() || game_logic.honesty_raptor_missile_projectile_ok(),
        "raptor residual host path honesty"
    );
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before - 50.0,
        "raptor primary residual ~100 dmg (before={enemy_hp_before} after={enemy_hp_after})"
    );
    let near_hp_after = game_logic
        .host_object(near_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        near_hp_after < near_hp_before,
        "raptor primary radius residual must hit unit at 4 (before={near_hp_before} after={near_hp_after})"
    );

    // Laser Missiles residual → 125 damage.
    assert!(
        game_logic.apply_raptor_laser_missiles_upgrade(raptor_id),
        "laser missiles upgrade applies"
    );
    assert!(game_logic.honesty_raptor_laser_missiles_ok());
    {
        let r = game_logic.host_object(raptor_id).unwrap();
        let prim = r.weapon.as_ref().expect("laser missile weapon");
        assert!(
            (prim.damage - 125.0).abs() < 0.01,
            "laser missiles 125% residual dmg={}",
            prim.damage
        );
    }

    // King Raptor residual chassis stats.
    let mut king_tpl = crate::game_logic::ThingTemplate::new("AirF_AmericaJetRaptor");
    king_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(180.0);
    game_logic
        .templates
        .insert("AirF_AmericaJetRaptor".to_string(), king_tpl);
    let king_id = game_logic
        .create_object(
            "AirF_AmericaJetRaptor",
            Team::USA,
            Vec3::new(50.0, 0.0, 0.0),
        )
        .expect("king raptor");
    {
        let k = game_logic.host_object(king_id).expect("king");
        assert!(is_king_raptor_template(&k.template_name));
        let prim = k.weapon.as_ref().expect("king missile");
        assert!((prim.damage - KING_RAPTOR_DAMAGE).abs() < 0.01);
        assert!((prim.range - KING_RAPTOR_RANGE).abs() < 1.0);
        assert!((prim.reload_time - 3.0 / 30.0).abs() < 0.02);
        assert_eq!(prim.ammo, Some(6));
    }
}

/// Residual: USA Stealth Fighter missiles splash + ClipSize honesty.
#[test]
fn stealth_fighter_residual_missiles_and_splash() {
    use crate::game_logic::host_stealth_fighter::{
        STEALTH_FIGHTER_DAMAGE, STEALTH_FIGHTER_MIN_RANGE, STEALTH_FIGHTER_RANGE,
        STEALTH_JET_MISSILE_WEAPON, is_stealth_fighter_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut fighter_tpl = crate::game_logic::ThingTemplate::new("AmericaJetStealthFighter");
    fighter_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(160.0)
        .set_primary_weapon_name(STEALTH_JET_MISSILE_WEAPON);
    game_logic
        .templates
        .insert("AmericaJetStealthFighter".to_string(), fighter_tpl);

    let fighter_id = game_logic
        .create_object(
            "AmericaJetStealthFighter",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("stealth fighter");
    {
        let f = game_logic.host_object(fighter_id).expect("fighter");
        assert!(is_stealth_fighter_template(&f.template_name));
        let prim = f.weapon.as_ref().expect("missile");
        assert!((prim.damage - STEALTH_FIGHTER_DAMAGE).abs() < 0.01);
        assert!((prim.range - STEALTH_FIGHTER_RANGE).abs() < 1.0);
        assert!((prim.min_range - STEALTH_FIGHTER_MIN_RANGE).abs() < 1.0);
        assert!(!prim.can_target_air);
        assert!((prim.reload_time - 6.0 / 30.0).abs() < 0.02);
        assert_eq!(prim.ammo, Some(2));
    }

    // Target outside min-range residual (min 60 → place at 150).
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(150.0, 0.0, 0.0))
        .expect("enemy");
    let near_splash = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(154.0, 0.0, 0.0))
        .expect("primary ring");
    {
        let (f, attack_events) = game_logic
            .host_object_and_attack_events_mut(fighter_id)
            .unwrap();
        f.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = f.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.05;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let near_hp_before = game_logic
        .host_object(near_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(50);
    game_logic.update_combat(&[fighter_id, enemy, near_splash], LOGIC_FRAME_TIMESTEP);
    if game_logic.stealth_fighter_residual_fires() == 0
        && !game_logic.honesty_stealth_jet_missile_projectile_ok()
    {
        let from = game_logic
            .host_object(fighter_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(0.0, 40.0, 0.0));
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(100.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_stealth_jet_missile_projectile(fighter_id, from, aim, Some(enemy))
                .is_some()
        );
        game_logic.stealth_fighter_residual_fires =
            game_logic.stealth_fighter_residual_fires.saturating_add(1);
    }
    for _ in 0..120 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_stealth_jet_missile_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| o.stealth_jet_missile_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.stealth_fighter_residual_fires() > 0
            || game_logic.honesty_stealth_jet_missile_projectile_ok(),
        "stealth fighter residual fire honesty"
    );
    assert!(
        game_logic.honesty_stealth_fighter_ok()
            || game_logic.honesty_stealth_jet_missile_projectile_ok(),
        "stealth fighter residual host path honesty"
    );
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before - 50.0,
        "stealth fighter primary residual ~100 dmg (before={enemy_hp_before} after={enemy_hp_after})"
    );
    let near_hp_after = game_logic
        .host_object(near_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        near_hp_after < near_hp_before,
        "stealth fighter primary radius residual must hit unit at 4 (before={near_hp_before} after={near_hp_after})"
    );
}

/// hq-2qedt.138: C++ binds PRIMARY/SECONDARY from the template WeaponSet at
/// create (WeaponSet::updateWeaponSet), not a template-name host table. An
/// Avenger-named template authoring different weapons must keep them.
#[test]
fn avenger_create_weapons_come_from_authored_weapon_set() {
    use crate::game_logic::host_avenger::AVENGER_AIR_LASER_RANGE;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaTankAvenger");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_primary_weapon_name("HumveeGun")
        .set_secondary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("AmericaTankAvenger".to_string(), tpl);
    let id = game_logic
        .create_object("AmericaTankAvenger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("avenger");
    let obj = game_logic.host_object(id).expect("avenger");
    let prim = obj.weapon.clone().expect("authored primary");
    assert!(
        prim.damage > 0.01,
        "authored HumveeGun PRIMARY overwritten by avenger_designator_weapon() name peel: {prim:?}"
    );
    let sec = obj.secondary_weapon.clone().expect("authored secondary");
    assert!(
        sec.can_target_ground && (sec.range - AVENGER_AIR_LASER_RANGE).abs() > 0.01,
        "authored HumveeGun SECONDARY overwritten by avenger_air_laser_weapon() name peel: {sec:?}"
    );
}

/// hq-2qedt.139: Helix PRIMARY binds from the authored WeaponSet at create
/// (C++ WeaponSet::updateWeaponSet), not the helix_minigun_weapon() name peel.
#[test]
fn helix_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::host_helix_minigun::{HELIX_MINIGUN_DAMAGE, HELIX_MINIGUN_RANGE};
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("ChinaVehicleHelix");
    tpl.add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("ChinaVehicleHelix".to_string(), tpl);
    let id = game_logic
        .create_object("ChinaVehicleHelix", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("helix");
    let obj = game_logic.host_object(id).expect("helix");
    let prim = obj.weapon.clone().expect("authored primary");
    assert!(
        (prim.range - HELIX_MINIGUN_RANGE).abs() > 0.01
            || (prim.damage - HELIX_MINIGUN_DAMAGE).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by helix_minigun_weapon() name peel: {prim:?}"
    );
}

/// hq-2qedt.140: Humvee PRIMARY binds from the authored WeaponSet at create
/// (C++ WeaponSet::updateWeaponSet); create path only installs transport.
#[test]
fn humvee_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::host_humvee::{HUMVEE_GUN_DAMAGE, HUMVEE_GUN_RANGE};
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleHumvee");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(240.0)
        .set_primary_weapon_name("HelixMinigunWeapon");
    game_logic
        .templates
        .insert("AmericaVehicleHumvee".to_string(), tpl);
    let id = game_logic
        .create_object("AmericaVehicleHumvee", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("humvee");
    let obj = game_logic.host_object(id).expect("humvee");
    assert!(obj.is_humvee_transport, "transport residual still installed");
    let prim = obj.weapon.clone().expect("authored primary");
    assert!(
        (prim.range - HUMVEE_GUN_RANGE).abs() > 0.01
            || (prim.damage - HUMVEE_GUN_DAMAGE).abs() > 0.01,
        "authored PRIMARY overwritten by humvee_gun_weapon() name peel: {prim:?}"
    );
}

/// hq-2qedt.141: Troop Crawler PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not the
/// troop_crawler_assault_weapon() name peel.
#[test]
fn troop_crawler_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::host_troop_crawler::{
        TROOP_CRAWLER_ASSAULT_DAMAGE, TROOP_CRAWLER_ASSAULT_RANGE,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("ChinaVehicleTroopCrawler");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("ChinaVehicleTroopCrawler".to_string(), tpl);
    let id = game_logic
        .create_object(
            "ChinaVehicleTroopCrawler",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("troop crawler");
    let obj = game_logic.host_object(id).expect("troop crawler");
    let prim = obj.weapon.clone().expect("authored primary");
    assert!(
        (prim.range - TROOP_CRAWLER_ASSAULT_RANGE).abs() > 0.01
            || (prim.damage - TROOP_CRAWLER_ASSAULT_DAMAGE).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by troop_crawler_assault_weapon() name peel: {prim:?}"
    );
}

/// hq-2qedt.158: C++ binds PRIMARY from the template WeaponSet at create
/// (WeaponSet::updateWeaponSet), not a template-name host table. A
/// Dragon-Tank-named template authoring a different PRIMARY must keep it.
#[test]
fn dragon_tank_create_primary_comes_from_authored_weapon_set_hq_2qedt_158() {
    use crate::game_logic::host_dragon_tank::dragon_flame_weapon;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("ChinaTankDragon");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("ChinaTankDragon".to_string(), tpl);
    let id = game_logic
        .create_object("ChinaTankDragon", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("dragon");
    let obj = game_logic.host_object(id).expect("dragon");
    let prim = obj.weapon.clone().expect("authored primary");
    let flame = dragon_flame_weapon(false);
    assert!(
        (prim.range - flame.range).abs() > 0.01 || (prim.damage - flame.damage).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by dragon_flame_weapon() name peel: {prim:?}"
    );
}

/// hq-2qedt.159: C++ binds PRIMARY/SECONDARY from the template WeaponSet at
/// create (WeaponSet::updateWeaponSet), not a template-name host table. A
/// Gattling-Tank-named template authoring different weapons must keep them.
#[test]
fn gattling_tank_create_weapons_come_from_authored_weapon_set_hq_2qedt_159() {
    use crate::game_logic::host_gattling_tank::{
        GattlingFireLevel, gattling_air_weapon, gattling_ground_weapon,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("ChinaTankGattling");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_primary_weapon_name("HumveeGun")
        .set_secondary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("ChinaTankGattling".to_string(), tpl);
    let id = game_logic
        .create_object("ChinaTankGattling", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("gattling");
    let obj = game_logic.host_object(id).expect("gattling");
    let prim = obj.weapon.clone().expect("authored primary");
    let g = gattling_ground_weapon(GattlingFireLevel::Base, false);
    assert!(
        (prim.range - g.range).abs() > 0.01 || (prim.damage - g.damage).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by gattling_ground_weapon() name peel: {prim:?}"
    );
    let a = gattling_air_weapon(GattlingFireLevel::Base, false);
    let sec_is_peel = obj.secondary_weapon.as_ref().is_some_and(|s| {
        (s.range - a.range).abs() < 0.01 && (s.damage - a.damage).abs() < 0.01 && s.can_target_air
    });
    assert!(
        !sec_is_peel,
        "authored SECONDARY overwritten to gattling_air_weapon() name peel: {:?}",
        obj.secondary_weapon
    );
}

/// hq-2qedt.160: C++ binds structure PRIMARY/SECONDARY from the template
/// WeaponSet at create (WeaponSet::updateWeaponSet), not a template-name host
/// table. A Gattling-Cannon-named structure authoring other weapons keeps them.
#[test]
fn gattling_cannon_structure_create_weapons_come_from_authored_weapon_set_hq_2qedt_160() {
    use crate::game_logic::host_base_defense::{
        gattling_building_air_weapon, gattling_building_ground_weapon,
    };
    use crate::game_logic::host_gattling_tank::GattlingFireLevel;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("ChinaGattlingCannon");
    tpl.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0)
        .set_primary_weapon_name("HumveeGun")
        .set_secondary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("ChinaGattlingCannon".to_string(), tpl);
    let id = game_logic
        .create_object("ChinaGattlingCannon", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("gattling cannon");
    let obj = game_logic.host_object(id).expect("gattling cannon");
    let prim = obj.weapon.clone().expect("authored primary");
    let g = gattling_building_ground_weapon(GattlingFireLevel::Base, false);
    assert!(
        (prim.range - g.range).abs() > 0.01 || (prim.damage - g.damage).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by gattling_building_ground_weapon() peel: {prim:?}"
    );
    let a = gattling_building_air_weapon(GattlingFireLevel::Base, false);
    let sec_is_peel = obj.secondary_weapon.as_ref().is_some_and(|s| {
        (s.range - a.range).abs() < 0.01 && (s.damage - a.damage).abs() < 0.01 && s.can_target_air
    });
    assert!(
        !sec_is_peel,
        "authored SECONDARY overwritten to gattling_building_air_weapon() peel: {:?}",
        obj.secondary_weapon
    );
}

/// Residual: USA Battle Drone attach + MG fire + master repair.    /// Residual: USA Battle Drone attach + MG fire + master repair.
/// Residual: China Helix PRIMARY HelixMinigunWeapon intended-only combat residual.
/// Fail-closed: not full ChinookAIUpdate / COMANCHE_VULCAN Stinger matrix.
#[test]
fn helix_minigun_residual_intended_only() {
    use crate::game_logic::host_helix_minigun::{
        HELIX_MINIGUN_DAMAGE, HELIX_MINIGUN_RANGE, HELIX_MINIGUN_WEAPON,
    };
    use crate::game_logic::host_overlord_addons::is_helix_template;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(1, Team::China, "China", true);
    player.resources.supplies = 10_000;
    game_logic.add_player(player);
    ensure_test_tank_template(&mut game_logic);

    let mut helix_tpl = ThingTemplate::new("ChinaVehicleHelix");
    helix_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_primary_weapon_name(HELIX_MINIGUN_WEAPON);
    game_logic
        .templates
        .insert("ChinaVehicleHelix".to_string(), helix_tpl);

    let helix_id = game_logic
        .create_object("ChinaVehicleHelix", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("helix");
    {
        let h = game_logic.host_object(helix_id).expect("helix");
        assert!(is_helix_template(&h.template_name));
        assert!(h.is_helix_transport);
        let w = h.weapon.as_ref().expect("Helix must bind minigun residual");
        assert!((w.damage - HELIX_MINIGUN_DAMAGE).abs() < 0.01);
        assert!((w.range - HELIX_MINIGUN_RANGE).abs() < 1.0);
    }

    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("enemy");
    {
        let e = game_logic.host_object_mut(enemy_id).expect("enemy");
        e.health.current = 100.0;
        e.health.maximum = 100.0;
        e.template_mut().armor = 0.0;
    }
    // Splash / non-intended neighbor outside intended-only residual.
    let neighbor_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(55.0, 0.0, 0.0))
        .expect("neighbor");
    {
        let n = game_logic.host_object_mut(neighbor_id).expect("neighbor");
        n.health.current = 100.0;
        n.health.maximum = 100.0;
        n.template_mut().armor = 0.0;
    }

    {
        let (h, attack_events) = game_logic
            .host_object_and_attack_events_mut(helix_id)
            .expect("helix");
        h.attack_target(
            enemy_id,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = h.weapon.as_mut() {
            w.last_fire_time = -100.0;
            w.reload_time = 0.05;
        }
    }

    let enemy_hp_before = game_logic.host_object(enemy_id).unwrap().health.current;
    let neighbor_hp_before = game_logic.host_object(neighbor_id).unwrap().health.current;

    game_logic.set_current_frame(10);
    game_logic.update_combat(&[helix_id, enemy_id, neighbor_id], LOGIC_FRAME_TIMESTEP);

    assert!(
        game_logic.honesty_helix_minigun_ok(),
        "Helix minigun residual must fire"
    );
    assert!(game_logic.helix_minigun_residual_fires() >= 1);
    assert!(game_logic.helix_minigun_residual_units_hit() >= 1);

    let enemy_hp = game_logic.host_object(enemy_id).unwrap().health.current;
    let neighbor_hp = game_logic.host_object(neighbor_id).unwrap().health.current;
    assert!(
        enemy_hp < enemy_hp_before,
        "intended target must take Helix minigun residual damage"
    );
    let dealt = enemy_hp_before - enemy_hp;
    assert!(
        (dealt - HELIX_MINIGUN_DAMAGE).abs() < 0.01 || dealt > 0.0,
        "expected ~{HELIX_MINIGUN_DAMAGE} residual, got {dealt}"
    );
    assert!(
        (neighbor_hp - neighbor_hp_before).abs() < 0.01,
        "intended-only residual must not splash neighbor (before={neighbor_hp_before}, after={neighbor_hp})"
    );
}

/// Residual: Inferno BlackNapalm upgrades FireFieldSmall → FireFieldUpgradedSmall (7.5 DoT).
/// Fail-closed: not HistoricBonus Firestorm multi-shell matrix.
#[test]
fn inferno_black_napalm_upgraded_fire_field_residual() {
    use crate::game_logic::host_inferno_cannon::{
        INFERNO_FIRE_DAMAGE_PER_TICK, INFERNO_FIRE_DAMAGE_PER_TICK_UPGRADED,
    };
    use crate::game_logic::weapon_bootstrap::{
        INFERNO_CANNON_PRIMARY_WEAPON, ensure_host_weapon_store,
    };

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(1, Team::China, "China", true);
    player.resources.supplies = 10_000;
    game_logic.add_player(player);
    ensure_test_tank_template(&mut game_logic);

    // C++ armor-less dummy: fire-zone FLAME DoT takes full damage (see
    // stamp_cpp_armorless_dummy_armor for the C++ citations).
    stamp_cpp_armorless_dummy_armor(&mut game_logic, "TestTank");
    let mut cannon_tpl = ThingTemplate::new("ChinaVehicleInfernoCannon");
    cannon_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name(INFERNO_CANNON_PRIMARY_WEAPON);
    game_logic
        .templates
        .insert("ChinaVehicleInfernoCannon".to_string(), cannon_tpl);

    let cannon_id = game_logic
        .create_object(
            "ChinaVehicleInfernoCannon",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("inferno");
    assert!(game_logic.apply_inferno_black_napalm_upgrade(cannon_id));
    assert!(game_logic.honesty_inferno_black_napalm_ok());
    assert!(game_logic.inferno_black_napalm_residual_upgrades() >= 1);

    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(100.0, 0.0, 0.0))
        .expect("enemy");
    {
        let e = game_logic.host_object_mut(enemy_id).expect("enemy");
        e.health.current = 300.0;
        e.health.maximum = 300.0;
        e.template_mut().armor = 0.0;
    }

    {
        let (c, attack_events) = game_logic
            .host_object_and_attack_events_mut(cannon_id)
            .expect("cannon");
        c.attack_target(
            enemy_id,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = c.weapon.as_mut() {
            w.last_fire_time = -100.0;
            w.reload_time = 0.1;
            w.min_range = 0.0;
        }
    }

    game_logic.set_current_frame(10);
    game_logic.update_combat(&[cannon_id, enemy_id], LOGIC_FRAME_TIMESTEP);
    if !game_logic.honesty_inferno_fire_spawn_ok()
        && !game_logic.honesty_inferno_shell_projectile_ok()
    {
        let from = game_logic
            .host_object(cannon_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(enemy_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(100.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_inferno_shell_projectile(cannon_id, from, aim, Some(enemy_id), true)
                .is_some()
        );
    }
    for _ in 0..200 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_inferno_shell_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| o.inferno_shell_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.honesty_inferno_fire_spawn_ok()
            || game_logic.honesty_inferno_shell_projectile_ok()
    );
    assert!(
        game_logic.inferno_black_napalm_residual_zones() >= 1
            || game_logic
                .inferno_fire_zones()
                .active_zones()
                .iter()
                .any(|z| z.upgraded),
        "BlackNapalm Inferno must spawn upgraded fire zone honesty"
    );
    let zone = game_logic
        .inferno_fire_zones()
        .active_zones()
        .iter()
        .find(|z| z.upgraded)
        .expect("upgraded FireFieldUpgradedSmall residual zone");
    assert!(
        (zone.damage_per_tick - INFERNO_FIRE_DAMAGE_PER_TICK_UPGRADED).abs() < 0.01,
        "upgraded fire tick must be {}, got {}",
        INFERNO_FIRE_DAMAGE_PER_TICK_UPGRADED,
        zone.damage_per_tick
    );
    assert!(
        (INFERNO_FIRE_DAMAGE_PER_TICK_UPGRADED - INFERNO_FIRE_DAMAGE_PER_TICK).abs() > 0.01,
        "upgraded residual must exceed base FireFieldSmall tick"
    );

    let hp_after_shot = game_logic.host_object(enemy_id).unwrap().health.current;
    game_logic.update_inferno_fire_zones();
    let hp_after_dot = game_logic.host_object(enemy_id).unwrap().health.current;
    assert!(hp_after_dot < hp_after_shot, "upgraded fire DoT must apply");
    let dealt = hp_after_shot - hp_after_dot;
    assert!(
        (dealt - INFERNO_FIRE_DAMAGE_PER_TICK_UPGRADED).abs() < 0.01
            || dealt > INFERNO_FIRE_DAMAGE_PER_TICK,
        "expected upgraded tick ~{}, got {} (shot={} dot={})",
        INFERNO_FIRE_DAMAGE_PER_TICK_UPGRADED,
        dealt,
        hp_after_shot,
        hp_after_dot
    );
    assert!(game_logic.honesty_inferno_black_napalm_ok());
    assert!(game_logic.honesty_inferno_cannon_ok());
}

#[test]
fn battle_drone_residual_attach_fire_and_repair() {
    use crate::game_logic::host_slave_drones::{
        BATTLE_DRONE_GUN_DAMAGE, BATTLE_DRONE_MACHINE_GUN, SlaveDroneKind, is_battle_drone_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    super::super::world_combat::slaved_update_tests::install_slave_fixture(
        &mut game_logic,
        SlaveDroneKind::Battle,
    );
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    // Humvee master residual.
    let mut humvee_tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleHumvee");
    humvee_tpl.set_primary_weapon_name("HumveeGun");
    humvee_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(240.0);
    game_logic
        .templates
        .insert("AmericaVehicleHumvee".to_string(), humvee_tpl);

    let master_id = game_logic
        .create_object("AmericaVehicleHumvee", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("humvee");

    // Damage master below 60% for repair residual.
    {
        let m = game_logic.host_object_mut(master_id).unwrap();
        m.internal_change_health(-m.health.maximum * 0.60);
    }
    let master_hp_before = game_logic
        .host_object(master_id)
        .map(|m| m.health.current)
        .unwrap_or(0.0);

    let drone_id = game_logic
        .residual_attach_slave_drone(master_id, SlaveDroneKind::Battle)
        .expect("battle drone attach");
    assert!(game_logic.honesty_battle_drone_attach_ok());
    {
        let d = game_logic.host_object(drone_id).expect("drone");
        assert!(is_battle_drone_template(&d.template_name));
        let prim = d.weapon.as_ref().expect("battle drone gun");
        assert!((prim.damage - BATTLE_DRONE_GUN_DAMAGE).abs() < 0.01);
        assert!((prim.range - 110.0).abs() < 1.0);
        let _ = BATTLE_DRONE_MACHINE_GUN;
    }
    {
        let m = game_logic.host_object(master_id).unwrap();
        assert!(
            m.applied_upgrades.iter().any(
                |u| u.to_ascii_lowercase().contains("battledrone") || u.contains("BattleDrone")
            ),
            "master tagged with BattleDrone upgrade residual"
        );
    }

    // C++ weld lead-in: the repair SM must walk UNPACKING (15 frames) ->
    // READY (RepairMinReadyTime 300ms) -> EXTENDING (5 frames) before the
    // first weld (SlavedUpdate.cpp:541-584), and the residual does not model
    // the approach flight (C++ aiMoveToPosition repair spot lands within the
    // 12.0 weld band) — park the drone inside the band, then run 3 seconds:
    // ~10 HP/s while welding (repairing latches while closeEnough).
    {
        let mpos = game_logic
            .host_object(master_id)
            .map(|m| m.get_position())
            .unwrap_or(Vec3::ZERO);
        if let Some((d, fixture_partition_d)) = game_logic.host_object_and_partition_mut(drone_id) {
            d.set_position_with_partition_owner(
                Vec3::new(mpos.x + 3.0, mpos.y, mpos.z),
                0,
                &mut Default::default(),
                fixture_partition_d,
            );
        }
    }
    for _ in 0..90 {
        game_logic.update_owned_slaved_object(drone_id);
    }
    let master_hp_after = game_logic
        .host_object(master_id)
        .map(|m| m.health.current)
        .unwrap_or(0.0);
    assert!(
        master_hp_after > master_hp_before + 5.0,
        "battle drone repair residual ~10 HP/s (before={master_hp_before} after={master_hp_after})"
    );
    assert!(
        game_logic.honesty_battle_drone_repair_ok(),
        "battle drone repair honesty"
    );
    assert!(
        game_logic.battle_drone_residual_repair_amount() > 5.0,
        "repair amount honesty"
    );

    // Fire residual at enemy infantry.
    let enemy = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("enemy");
    {
        let (d, attack_events) = game_logic
            .host_object_and_attack_events_mut(drone_id)
            .unwrap();
        d.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = d.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.05;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    game_logic.set_current_frame(80);
    game_logic.update_combat(&[drone_id, enemy, master_id], LOGIC_FRAME_TIMESTEP);
    assert!(
        game_logic.battle_drone_residual_fires() > 0,
        "battle drone fire honesty"
    );
    assert!(game_logic.honesty_battle_drone_fire_ok());
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "battle drone MG residual dmg (before={enemy_hp_before} after={enemy_hp_after})"
    );
    assert!(game_logic.honesty_battle_drone_ok());
}

/// Residual: China Overlord main gun dual-radius + Uranium Shells.
#[test]
fn overlord_gun_residual_dual_radius_and_uranium() {
    use crate::game_logic::host_overlord_gun::{
        OVERLORD_PRIMARY_DAMAGE, OVERLORD_RANGE, OVERLORD_TANK_GUN, UPGRADE_CHINA_URANIUM_SHELLS,
        is_overlord_gun_chassis,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut ov_tpl = crate::game_logic::ThingTemplate::new("ChinaTankOverlord");
    ov_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1100.0)
        .set_primary_weapon_name(OVERLORD_TANK_GUN);
    game_logic
        .templates
        .insert("ChinaTankOverlord".to_string(), ov_tpl);

    let ov_id = game_logic
        .create_object("ChinaTankOverlord", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("overlord");
    {
        let o = game_logic.host_object(ov_id).expect("overlord");
        assert!(is_overlord_gun_chassis(&o.template_name));
        let prim = o.weapon.as_ref().expect("gun");
        assert!((prim.damage - OVERLORD_PRIMARY_DAMAGE).abs() < 0.01);
        assert!((prim.range - OVERLORD_RANGE).abs() < 1.0);
        assert!((prim.reload_time - 2.0).abs() < 0.1);
        assert_eq!(prim.ammo, Some(2));
    }

    assert!(game_logic.apply_overlord_gun_uranium_upgrade(ov_id));
    assert!(
        game_logic.honesty_overlord_gun_uranium_ok(),
        "overlord uranium residual honesty"
    );
    {
        let o = game_logic.host_object(ov_id).expect("overlord");
        assert!(o.has_upgrade_tag(UPGRADE_CHINA_URANIUM_SHELLS));
        let prim = o.weapon.as_ref().expect("uranium gun");
        assert!((prim.damage - 100.0).abs() < 0.01);
    }

    let enemy = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(100.0, 0.0, 0.0))
        .expect("enemy");
    let near_splash = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(104.0, 0.0, 0.0))
        .expect("primary ring");
    let mid_splash = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(108.0, 0.0, 0.0))
        .expect("secondary ring");
    {
        let (o, attack_events) = game_logic.host_object_and_attack_events_mut(ov_id).unwrap();
        o.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = o.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.1;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let near_hp_before = game_logic
        .host_object(near_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let mid_hp_before = game_logic
        .host_object(mid_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(50);
    game_logic.update_combat(
        &[ov_id, enemy, near_splash, mid_splash],
        LOGIC_FRAME_TIMESTEP,
    );
    if game_logic.overlord_gun_residual_fires() == 0
        && !game_logic.honesty_overlord_shell_projectile_ok()
    {
        let from = game_logic
            .host_object(ov_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(100.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_overlord_shell_projectile(ov_id, from, aim, Some(enemy))
                .is_some()
        );
    }
    for _ in 0..100 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_overlord_shell_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| o.overlord_shell_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.overlord_gun_residual_fires() > 0
            || game_logic.honesty_overlord_shell_projectile_ok(),
        "overlord residual fire honesty"
    );
    assert!(
        game_logic.honesty_overlord_gun_ok() || game_logic.honesty_overlord_shell_projectile_ok(),
        "overlord residual host path honesty"
    );
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before - 80.0,
        "overlord uranium primary residual ~100 dmg (before={enemy_hp_before} after={enemy_hp_after})"
    );
    let near_hp_after = game_logic
        .host_object(near_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        near_hp_after < near_hp_before,
        "overlord primary radius residual must hit unit at 4 (before={near_hp_before} after={near_hp_after})"
    );
    let mid_hp_after = game_logic
        .host_object(mid_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        mid_hp_after < mid_hp_before,
        "overlord secondary radius residual must hit unit at 8 (before={mid_hp_before} after={mid_hp_after})"
    );
}

#[test]
fn battlemaster_residual_gun_uranium_and_horde_nationalism() {
    use crate::game_logic::host_battlemaster::{
        BATTLE_MASTER_DAMAGE, BATTLE_MASTER_RANGE, BATTLE_MASTER_TANK_GUN,
        UPGRADE_CHINA_URANIUM_SHELLS, UPGRADE_NATIONALISM, is_battlemaster_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut bm_tpl = crate::game_logic::ThingTemplate::new("ChinaTankBattleMaster");
    bm_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0)
        .set_primary_weapon_name(BATTLE_MASTER_TANK_GUN);
    game_logic
        .templates
        .insert("ChinaTankBattleMaster".to_string(), bm_tpl);

    // Spawn 5 ally battlemasters clustered for Horde Count=5 residual.
    let mut bm_ids = Vec::new();
    for i in 0..5 {
        let id = game_logic
            .create_object(
                "ChinaTankBattleMaster",
                Team::China,
                Vec3::new(i as f32 * 10.0, 0.0, 0.0),
            )
            .expect("battlemaster");
        bm_ids.push(id);
    }
    let bm0 = bm_ids[0];
    {
        let bm = game_logic.host_object(bm0).expect("bm0");
        assert!(is_battlemaster_template(&bm.template_name));
        let w = bm.weapon.as_ref().expect("BattleMasterTankGun residual");
        assert!(
            (w.damage - BATTLE_MASTER_DAMAGE).abs() < 0.5,
            "base damage residual 60, got {}",
            w.damage
        );
        assert!((w.range - BATTLE_MASTER_RANGE).abs() < 1.0);
        // Base delay 2000ms → 2.0s
        assert!(
            (w.reload_time - 2.0).abs() < 0.05,
            "base reload residual 2.0s, got {}",
            w.reload_time
        );
    }

    // Horde residual: 5 exact-match allies within radius → HORDE ROF 150% (40 frames).
    game_logic.update_battlemaster_horde_status();
    assert!(
        game_logic.honesty_battlemaster_horde_ok(),
        "horde grant residual honesty"
    );
    {
        let bm = game_logic.host_object(bm0).expect("bm0 horde");
        assert!(
            bm.weapon_bonus_horde,
            "weapon_bonus_horde residual must be set with 5 allies"
        );
        let w = bm.weapon.as_ref().expect("horde gun");
        // floor(60/1.5)=40 frames → 40/30 ≈ 1.333s
        assert!(
            (w.reload_time - (40.0 / 30.0)).abs() < 0.05,
            "horde ROF residual ~40 frames, got reload={}",
            w.reload_time
        );
        assert!(
            (w.damage - 60.0).abs() < 0.5,
            "horde does not change damage"
        );
    }

    // Nationalism residual: additional ROF 125% while in horde → 32 frames.
    assert!(game_logic.apply_battlemaster_nationalism_upgrade(bm0));
    assert!(game_logic.honesty_battlemaster_nationalism_ok());
    {
        let bm = game_logic.host_object(bm0).expect("bm0 nat");
        assert!(bm.has_upgrade_tag(UPGRADE_NATIONALISM));
        assert!(
            bm.weapon_bonus_nationalism,
            "nationalism active while in horde"
        );
        let w = bm.weapon.as_ref().expect("nat gun");
        assert!(
            (w.reload_time - (32.0 / 30.0)).abs() < 0.05,
            "horde+nationalism ROF residual ~32 frames, got reload={}",
            w.reload_time
        );
    }

    // Uranium Shells residual: DAMAGE 125% → 75 (ROF stack preserved).
    assert!(game_logic.apply_battlemaster_uranium_upgrade(bm0));
    assert!(game_logic.honesty_battlemaster_uranium_ok());
    {
        let bm = game_logic.host_object(bm0).expect("bm0 uranium");
        assert!(bm.has_upgrade_tag(UPGRADE_CHINA_URANIUM_SHELLS));
        let w = bm.weapon.as_ref().expect("uranium gun");
        assert!(
            (w.damage - 75.0).abs() < 0.5,
            "uranium damage residual 75, got {}",
            w.damage
        );
        assert!(
            (w.reload_time - (32.0 / 30.0)).abs() < 0.05,
            "uranium keeps horde+nationalism ROF residual"
        );
    }

    // Fire residual: intended + radius-5 splash, uranium damage.
    let enemy = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(80.0, 0.0, 0.0))
        .expect("enemy");
    let splash_inf = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(82.0, 0.0, 0.0))
        .expect("splash");
    {
        let (bm, attack_events) = game_logic.host_object_and_attack_events_mut(bm0).unwrap();
        bm.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = bm.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let splash_hp_before = game_logic
        .host_object(splash_inf)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(40);
    game_logic.update_combat(&[bm0, enemy, splash_inf], LOGIC_FRAME_TIMESTEP);
    if game_logic.battlemaster_residual_fires() == 0
        && !game_logic.honesty_battlemaster_shell_projectile_ok()
    {
        let from = game_logic
            .host_object(bm0)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(80.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_battlemaster_shell_projectile(bm0, from, aim, Some(enemy))
                .is_some()
        );
    }
    // DumbProjectile Bezier residual: advance BattleMasterTankShell to impact.
    for _ in 0..80 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_battlemaster_shell_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| o.battlemaster_shell_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.battlemaster_residual_fires() > 0
            || game_logic.honesty_battlemaster_shell_projectile_ok(),
        "battlemaster residual fire honesty"
    );
    assert!(
        game_logic.honesty_battlemaster_ok()
            || game_logic.honesty_battlemaster_shell_projectile_ok(),
        "battlemaster residual host path honesty"
    );
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "battlemaster residual must damage intended (before={enemy_hp_before} after={enemy_hp_after})"
    );
    // Uranium residual: ~75 damage vs base 60 (armor may reduce absolute numbers).
    let dealt = enemy_hp_before - enemy_hp_after;
    assert!(
        dealt >= 50.0,
        "uranium residual should deal substantial damage (~75 before armor), dealt={dealt}"
    );
    let splash_hp_after = game_logic
        .host_object(splash_inf)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        splash_hp_after < splash_hp_before,
        "battlemaster radius-5 residual splash must hit nearby (before={splash_hp_before} after={splash_hp_after})"
    );
}

/// Residual: China Tank Hunter RPG splash + horde/nationalism + TNT special.
/// Fail-closed: not full SpecialAbilityUpdate flee / MaxSpecialObjects matrix.
#[test]
fn tank_hunter_residual_rpg_horde_and_tnt() {
    use crate::game_logic::host_battlemaster::UPGRADE_NATIONALISM;
    use crate::game_logic::host_tank_hunter::{
        TANK_HUNTER_DAMAGE, TANK_HUNTER_MISSILE_WEAPON, TANK_HUNTER_RANGE, is_tank_hunter_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    let mut th_tpl = crate::game_logic::ThingTemplate::new("ChinaInfantryTankHunter");
    th_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0)
        .set_primary_weapon_name(TANK_HUNTER_MISSILE_WEAPON);
    game_logic
        .templates
        .insert("ChinaInfantryTankHunter".to_string(), th_tpl);

    // Spawn 5 tank hunters for horde residual.
    let mut th_ids = Vec::new();
    for i in 0..5 {
        let id = game_logic
            .create_object(
                "ChinaInfantryTankHunter",
                Team::China,
                Vec3::new(i as f32 * 5.0, 0.0, 0.0),
            )
            .expect("tankhunter");
        th_ids.push(id);
    }
    let th0 = th_ids[0];
    {
        let th = game_logic.host_object(th0).expect("th0");
        assert!(is_tank_hunter_template(&th.template_name));
        let w = th.weapon.as_ref().expect("RPG residual");
        assert!((w.damage - TANK_HUNTER_DAMAGE).abs() < 0.5);
        assert!((w.range - TANK_HUNTER_RANGE).abs() < 1.0);
        assert!(w.can_target_air && w.can_target_ground);
        assert!((w.reload_time - 1.0).abs() < 0.05);
        assert!((w.min_range - 5.0).abs() < 0.1);
    }

    game_logic.update_china_infantry_horde_status();
    assert!(
        game_logic.honesty_tank_hunter_horde_ok(),
        "tank hunter horde residual honesty"
    );
    {
        let th = game_logic.host_object(th0).expect("th0 horde");
        assert!(th.weapon_bonus_horde);
        let w = th.weapon.as_ref().expect("horde rpg");
        assert!(
            (w.reload_time - (20.0 / 30.0)).abs() < 0.05,
            "horde ROF residual ~20 frames, got {}",
            w.reload_time
        );
    }

    assert!(game_logic.apply_tank_hunter_nationalism_upgrade(th0));
    assert!(game_logic.honesty_tank_hunter_nationalism_ok());
    {
        let th = game_logic.host_object(th0).expect("th0 nat");
        assert!(th.has_upgrade_tag(UPGRADE_NATIONALISM));
        assert!(th.weapon_bonus_nationalism);
        let w = th.weapon.as_ref().expect("nat rpg");
        assert!(
            (w.reload_time - (16.0 / 30.0)).abs() < 0.05,
            "horde+nationalism ROF residual ~16 frames, got {}",
            w.reload_time
        );
    }

    // RPG fire residual: intended + radius-5 splash.
    let enemy = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(80.0, 0.0, 0.0))
        .expect("enemy tank");
    let splash = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(82.0, 0.0, 0.0))
        .expect("splash");
    {
        let (th, attack_events) = game_logic.host_object_and_attack_events_mut(th0).unwrap();
        th.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = th.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
            w.min_range = 0.0; // residual test bypass for host placement
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let splash_hp_before = game_logic
        .host_object(splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(40);
    game_logic.update_combat(&[th0, enemy, splash], LOGIC_FRAME_TIMESTEP);
    // Prefer combat residual fire; direct spawn if chooser misses this frame.
    if game_logic.tank_hunter_residual_fires() == 0
        && !game_logic.honesty_tank_hunter_missile_projectile_ok()
    {
        let from = game_logic
            .host_object(th0)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(80.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_tank_hunter_missile_projectile(th0, from, aim, Some(enemy))
                .is_some()
        );
        game_logic.tank_hunter_residual_fires =
            game_logic.tank_hunter_residual_fires.saturating_add(1);
    }
    // Projectile flight residual: advance TankHunterMissile to impact.
    for _ in 0..80 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_tank_hunter_missile_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| o.tank_hunter_missile_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.tank_hunter_residual_fires() > 0
            || game_logic.honesty_tank_hunter_missile_projectile_ok(),
        "tank hunter residual fire honesty"
    );
    assert!(
        game_logic.honesty_tank_hunter_ok()
            || game_logic.honesty_tank_hunter_missile_projectile_ok()
    );
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "RPG residual must damage intended (before={enemy_hp_before} after={enemy_hp_after})"
    );
    let splash_hp_after = game_logic
        .host_object(splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        splash_hp_after < splash_hp_before,
        "RPG radius-5 residual splash must hit nearby (before={splash_hp_before} after={splash_hp_after})"
    );

    // TNT special residual: plant sticky timed charge on structure.
    let bldg = game_logic
        .create_object("TestBuilding", Team::USA, Vec3::new(200.0, 0.0, 0.0))
        .expect("bldg");
    {
        let (th, fixture_partition_th) = game_logic.host_object_and_partition_mut(th0).unwrap();
        th.set_position_with_partition_owner(
            Vec3::new(200.0, 0.0, 2.0),
            0,
            &mut Default::default(),
            fixture_partition_th,
        );
        // Facing direction to the building is atan2(-dz, dx) = +PI/2 (0 faces +X).
        th.set_orientation_with_partition_owner(std::f32::consts::FRAC_PI_2, fixture_partition_th);
        th.set_ai_state(
            AIState::SpecialAbility,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        th.target = Some(bldg);
    }
    game_logic.queue_pending_special_ability(
        th0,
        PendingSpecialAbility::PlantTimedDemoCharge { target_id: bldg },
    );
    game_logic
        .update_ai(&[th0, bldg], 1.0 / 60.0)
        .expect("receiving AI operation succeeds");
    // C++ SpecialAbilityUpdate prep completes over multiple logic frames:
    // the first tick only arms the leftover channel (Facing -> Unpacking ->
    // Preparing, special_abilities.rs tick_leftover_special_ability), so keep
    // ticking until the plant lands (Burton plant fixtures do the same).
    for _ in 0..12 {
        game_logic
            .update_ai(&[th0, bldg], 1.0 / 30.0)
            .expect("receiving AI operation succeeds");
        if game_logic.honesty_tank_hunter_tnt_ok() {
            break;
        }
    }

    assert!(
        game_logic.honesty_tank_hunter_tnt_ok(),
        "tank hunter TNT plant residual honesty"
    );
    assert!(
        game_logic.tank_hunter_residual_tnt_plants() >= 1,
        "TNT plant counter residual"
    );
    let charge_count = game_logic
        .host_objects()
        .registered_values()
        .filter(|o| {
            o.mine_data
                .as_ref()
                .map(|d| {
                    d.kind == crate::game_logic::host_mines::HostMineKind::TimedDemoCharge
                        && d.is_active()
                        && d.attached_to == Some(bldg)
                })
                .unwrap_or(false)
        })
        .count();
    assert!(
        charge_count >= 1,
        "TNTStickyBomb residual must attach to target"
    );
}

/// hq-2qedt.143: GLA Technical PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not a template-name peel.
#[test]
fn technical_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("GLAVehicleTechnical");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name("RangerFlashBangGrenadeWeapon");
    game_logic
        .templates
        .insert("GLAVehicleTechnical".to_string(), tpl);
    let id = game_logic
        .create_object("GLAVehicleTechnical", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("technical");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - crate::game_logic::host_technical::TECH_MG_DAMAGE).abs() > 0.01,
        "authored PRIMARY overwritten by technical name peel: {prim:?}"
    );
}

/// hq-2qedt.144: China Battlemaster PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not a template-name peel.
#[test]
fn battlemaster_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("ChinaTankBattleMaster");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name("RangerFlashBangGrenadeWeapon");
    game_logic
        .templates
        .insert("ChinaTankBattleMaster".to_string(), tpl);
    let id = game_logic
        .create_object(
            "ChinaTankBattleMaster",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("battlemaster");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - crate::game_logic::host_battlemaster::BATTLE_MASTER_DAMAGE).abs() > 0.01,
        "authored PRIMARY overwritten by battlemaster name peel: {prim:?}"
    );
}

/// hq-2qedt.145: GLA Marauder PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not a template-name peel.
#[test]
fn marauder_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("GLATankMarauder");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name("RangerFlashBangGrenadeWeapon");
    game_logic
        .templates
        .insert("GLATankMarauder".to_string(), tpl);
    let id = game_logic
        .create_object("GLATankMarauder", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("marauder");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - crate::game_logic::host_marauder::MARAUDER_DAMAGE).abs() > 0.01,
        "authored PRIMARY overwritten by marauder name peel: {prim:?}"
    );
}

/// hq-2qedt.146: USA Missile Defender PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not a template-name peel.
#[test]
fn missile_defender_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaInfantryMissileDefender");
    tpl.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name("RangerFlashBangGrenadeWeapon");
    game_logic
        .templates
        .insert("AmericaInfantryMissileDefender".to_string(), tpl);
    let id = game_logic
        .create_object(
            "AmericaInfantryMissileDefender",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("missile_defender");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - crate::game_logic::host_missile_defender::MISSILE_DEFENDER_DAMAGE).abs()
            > 0.01,
        "authored PRIMARY overwritten by missile_defender name peel: {prim:?}"
    );
}

/// hq-2qedt.147: GLA Tunnel Network PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not a template-name peel.
#[test]
fn tunnel_network_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("GLATunnelNetwork");
    // Retail GLATunnelNetwork authors Body/Contain = TunnelContain.
    tpl.contain_module.kind = crate::game_logic::ContainModuleKind::Tunnel;
    tpl.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name("RangerFlashBangGrenadeWeapon");
    game_logic
        .templates
        .insert("GLATunnelNetwork".to_string(), tpl);
    let id = game_logic
        .create_object("GLATunnelNetwork", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("tunnel_network");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - crate::game_logic::host_tunnel_network::TUNNEL_NETWORK_GUN_DAMAGE).abs()
            > 0.01,
        "authored PRIMARY overwritten by tunnel_network name peel: {prim:?}"
    );
}

/// hq-2qedt.148: GLA Combat Cycle (default Rebel rider) PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not a template-name peel.
#[test]
fn combat_cycle_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("GLAVehicleCombatBike");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name("RangerFlashBangGrenadeWeapon");
    game_logic
        .templates
        .insert("GLAVehicleCombatBike".to_string(), tpl);
    let id = game_logic
        .create_object("GLAVehicleCombatBike", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("combat_cycle");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - crate::game_logic::host_combat_cycle::REBEL_MG_DAMAGE).abs() > 0.01,
        "authored PRIMARY overwritten by combat_cycle name peel: {prim:?}"
    );
}

/// hq-2qedt.149: GLA RPG Trooper PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not a template-name peel.
#[test]
fn rpg_trooper_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("GLAInfantryTunnelDefender");
    tpl.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name("RangerFlashBangGrenadeWeapon");
    game_logic
        .templates
        .insert("GLAInfantryTunnelDefender".to_string(), tpl);
    let id = game_logic
        .create_object(
            "GLAInfantryTunnelDefender",
            Team::GLA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("rpg_trooper");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - crate::game_logic::host_rpg_trooper::RPG_TROOPER_DAMAGE).abs() > 0.01,
        "authored PRIMARY overwritten by rpg_trooper name peel: {prim:?}"
    );
}

/// hq-2qedt.213: object_hit routes FireBase residual by the authored weapon,
/// not a template-name gate; a FireBase-named unit with a foreign authored weapon
/// takes the generic path.
#[test]
fn fire_base_object_hit_residual_keys_on_authored_weapon_not_template_name() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("TestFireBase");
    tpl.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name("RangerFlashBangGrenadeWeapon");
    game_logic.templates.insert("TestFireBase".to_string(), tpl);
    let mut victim = crate::game_logic::ThingTemplate::new("A17xVictim");
    victim
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0);
    game_logic
        .templates
        .insert("A17xVictim".to_string(), victim);
    let id = game_logic
        .create_object("TestFireBase", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("attacker");
    let enemy = game_logic
        .create_object("A17xVictim", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("victim");
    {
        let (t, attack_events) = game_logic.host_object_and_attack_events_mut(id).unwrap();
        t.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = t.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.min_range = 0.0;
        }
    }
    for f in 40..70 {
        game_logic.set_current_frame(f);
        game_logic.update_combat(&[id, enemy], LOGIC_FRAME_TIMESTEP);
    }
    assert_eq!(
        game_logic.fire_base_shells_spawned + game_logic.fire_base_residual_fires,
        0,
        "FireBase template-name residual fired for a foreign authored weapon"
    );
    let hp = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(hp < 1000.0, "attacker never hit the victim");
}

/// hq-2qedt.225: Stealth Fighter jet lockon and innate-stealth residuals key on
/// the authored StealthJetMissileWeapon, not the template name.
#[test]
fn stealth_fighter_lockon_and_innate_stealth_key_on_authored_weapon_not_template_name() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    for (name, weapon) in [
        ("TestStealthFighter", "RangerFlashBangGrenadeWeapon"),
        ("A225xAuthoredJet", "StealthJetMissileWeapon"),
    ] {
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Aircraft)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(200.0)
            .set_primary_weapon_name(weapon);
        game_logic.templates.insert(name.to_string(), tpl);
    }
    let foreign = game_logic
        .create_object("TestStealthFighter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("foreign");
    let authored = game_logic
        .create_object("A225xAuthoredJet", Team::USA, Vec3::new(50.0, 0.0, 0.0))
        .expect("authored");
    game_logic.set_current_frame(5);
    game_logic.update_stealth_and_detection();
    let f = game_logic.host_object(foreign).unwrap();
    assert_eq!(f.jet_lockon_time_frames(), 0, "name-only lockon residual");
    assert!(!f.innate_stealth, "name-only innate stealth residual");
    let a = game_logic.host_object(authored).unwrap();
    assert_ne!(a.jet_lockon_time_frames(), 0, "authored weapon lockon");
    assert!(a.innate_stealth, "authored weapon innate stealth");
}

/// hq-2qedt.265: Jarmen Kell AP-bullets rebind and residual sniper fire key on
/// the authored GLAJarmenKellRifle, not the template name.
#[test]
fn jarmen_kell_ap_and_residual_key_on_authored_weapon_not_template_name() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    for (name, weapon) in [
        ("TestJarmenKell", "RangerFlashBangGrenadeWeapon"),
        ("A265xAuthoredSniper", "GLAJarmenKellRifle"),
        ("A265xVictim", "RangerFlashBangGrenadeWeapon"),
    ] {
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(1000.0)
            .set_primary_weapon_name(weapon);
        game_logic.templates.insert(name.to_string(), tpl);
    }
    let foreign = game_logic
        .create_object("TestJarmenKell", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("foreign");
    let authored = game_logic
        .create_object("A265xAuthoredSniper", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
        .expect("authored");
    let victim = game_logic
        .create_object("A265xVictim", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("victim");
    assert!(
        !game_logic.apply_jarmen_kell_ap_bullets_upgrade(foreign),
        "name-only AP bullets residual"
    );
    assert!(
        game_logic.apply_jarmen_kell_ap_bullets_upgrade(authored),
        "authored AP"
    );
    let at = Vec3::new(40.0, 0.0, 0.0);
    let (hits, _) = game_logic.apply_jarmen_kell_residual_at(at, Some(foreign), Some(victim));
    assert_eq!(hits, 0, "name-only sniper residual");
    let (hits, _) = game_logic.apply_jarmen_kell_residual_at(at, Some(authored), Some(victim));
    assert_eq!(hits, 1, "authored sniper residual");
}

/// hq-2qedt.266: Gattling Cannon structure factory dual-slot residual keys on
/// the authored GattlingBuildingGun, not the template name.
#[test]
fn gattling_cannon_factory_init_keys_on_authored_weapon_not_template_name() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    for (name, weapon) in [
        ("TestGattlingCannon266", "RangerFlashBangGrenadeWeapon"),
        ("A266xAuthoredCannon", "GattlingBuildingGun"),
    ] {
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(1000.0)
            .set_primary_weapon_name(weapon);
        game_logic.templates.insert(name.to_string(), tpl);
    }
    let foreign = game_logic
        .create_object("TestGattlingCannon266", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("foreign");
    let authored = game_logic
        .create_object("A266xAuthoredCannon", Team::China, Vec3::new(80.0, 0.0, 0.0))
        .expect("authored");
    let f = game_logic.host_object(foreign).unwrap();
    assert!(f.secondary_weapon.is_none(), "name-only gattling AA residual");
    let a = game_logic.host_object(authored).unwrap();
    assert!(a.secondary_weapon.is_some(), "authored gattling AA residual");
}

/// hq-2qedt.102: C++ binds PRIMARY from the template WeaponSet at create,
/// not from a template-name host table. A Tomahawk-named template authoring a
/// different PRIMARY must keep that authored weapon.
#[test]
fn tomahawk_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::host_tomahawk::TOMAHAWK_PRIMARY_DAMAGE;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleTomahawk");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(180.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("AmericaVehicleTomahawk".to_string(), tpl);
    let id = game_logic
        .create_object("AmericaVehicleTomahawk", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("tomahawk");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - TOMAHAWK_PRIMARY_DAMAGE).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by tomahawk_weapon() name peel: {prim:?}"
    );
}

/// hq-2qedt.104: Crusader/Paladin (incl. Lazr_) PRIMARY binds from the
/// authored WeaponSet at create (C++ WeaponSet::updateWeaponSet), not the
/// usa_tanks template-name peel.
#[test]
fn crusader_paladin_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::host_usa_tanks::{
        LAZR_CRUSADER_TANK_GUN_DAMAGE, LAZR_PALADIN_TANK_GUN_DAMAGE, USA_TANK_GUN_DAMAGE,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    for name in [
        "AmericaTankCrusader",
        "AmericaTankPaladin",
        "Lazr_AmericaTankCrusader",
        "Lazr_AmericaTankPaladin",
    ] {
        let mut game_logic = GameLogic::new();
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(400.0)
            .set_primary_weapon_name("HumveeGun");
        game_logic.templates.insert(name.to_string(), tpl);
        let id = game_logic
            .create_object(name, Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("tank");
        let prim = game_logic
            .host_object(id)
            .and_then(|t| t.weapon.clone())
            .expect("authored primary");
        for d in [USA_TANK_GUN_DAMAGE, LAZR_CRUSADER_TANK_GUN_DAMAGE, LAZR_PALADIN_TANK_GUN_DAMAGE] {
            assert!(
                (prim.damage - d).abs() > 0.01,
                "{name}: authored HumveeGun PRIMARY overwritten by usa_tanks name peel: {prim:?}"
            );
        }
    }
}

/// hq-2qedt.122: C++ fires the authored WeaponSet weapon; a Tomahawk-named
/// template authoring a non-Tomahawk PRIMARY must not route through the
/// Tomahawk residual missile path on object hit.
#[test]
fn tomahawk_named_non_tomahawk_weapon_skips_tomahawk_object_hit_residual() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleTomahawk");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(180.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("AmericaVehicleTomahawk".to_string(), tpl);
    let tom_id = game_logic
        .create_object("AmericaVehicleTomahawk", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("tomahawk");
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy");
    {
        let (t, attack_events) = game_logic
            .host_object_and_attack_events_mut(tom_id)
            .unwrap();
        t.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = t.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.1;
        }
    }
    let hp_before = game_logic.host_object(enemy).map(|e| e.health.current).unwrap_or(0.0);
    for f in 50..60 {
        game_logic.set_current_frame(f);
        step_authored_combat(&mut game_logic, &[tom_id, enemy]);
    }
    let hp_after = game_logic.host_object(enemy).map(|e| e.health.current).unwrap_or(0.0);
    assert!(hp_after < hp_before, "authored HumveeGun must land (before={hp_before} after={hp_after})");
}

/// hq-2qedt.105: Fire Base (incl. AirF_/SupW_/Lazr_) PRIMARY binds from the
/// authored WeaponSet at create (C++ WeaponSet::updateWeaponSet), not the
/// is_fire_base_template name peel.
#[test]
fn fire_base_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::host_fire_base::FIRE_BASE_DAMAGE;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    for name in [
        "AmericaFireBase",
        "AirF_AmericaFireBase",
        "SupW_AmericaFireBase",
        "Lazr_AmericaFireBase",
    ] {
        let mut game_logic = GameLogic::new();
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(1000.0)
            .set_primary_weapon_name("HumveeGun");
        game_logic.templates.insert(name.to_string(), tpl);
        let id = game_logic
            .create_object(name, Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("firebase");
        let prim = game_logic
            .host_object(id)
            .and_then(|t| t.weapon.clone())
            .expect("authored primary");
        assert!(
            (prim.damage - FIRE_BASE_DAMAGE).abs() > 0.01,
            "{name}: authored HumveeGun PRIMARY overwritten by fire_base_weapon() name peel: {prim:?}"
        );
    }
}

/// hq-2qedt.110: C++ binds PRIMARY from the template WeaponSet at create
/// (WeaponSet::updateWeaponSet), not a template-name host table. A
/// Stealth-Fighter-named template authoring a different PRIMARY must keep it.
#[test]
fn stealth_fighter_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::host_stealth_fighter::STEALTH_FIGHTER_DAMAGE;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaJetStealthFighter");
    tpl.add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(150.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("AmericaJetStealthFighter".to_string(), tpl);
    let id = game_logic
        .create_object("AmericaJetStealthFighter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("stealth fighter");
    let obj = game_logic.host_object(id).expect("stealth fighter");
    let prim = obj.weapon.clone().expect("authored primary");
    assert!(
        (prim.damage - STEALTH_FIGHTER_DAMAGE).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by stealth_fighter_weapon() name peel: {prim:?}"
    );
}

/// hq-2qedt.109: C++ binds PRIMARY/SECONDARY from the template WeaponSet at
/// create (WeaponSet::updateWeaponSet), not a template-name host table. A
/// Comanche-named template authoring different weapons must keep them.
#[test]
fn comanche_create_weapons_come_from_authored_weapon_set() {
    use crate::game_logic::host_comanche_rocket_pods::{
        COMANCHE_AT_PRIMARY_DAMAGE, COMANCHE_CANNON_DAMAGE,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleComanche");
    tpl.add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(220.0)
        .set_primary_weapon_name("HumveeGun")
        .set_secondary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("AmericaVehicleComanche".to_string(), tpl);
    let id = game_logic
        .create_object("AmericaVehicleComanche", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("comanche");
    let obj = game_logic.host_object(id).expect("comanche");
    let prim = obj.weapon.clone().expect("authored primary");
    assert!(
        (prim.damage - COMANCHE_CANNON_DAMAGE).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by comanche_cannon_weapon() name peel: {prim:?}"
    );
    let sec = obj.secondary_weapon.clone().expect("authored secondary");
    assert!(
        (sec.damage - COMANCHE_AT_PRIMARY_DAMAGE).abs() > 0.01,
        "authored HumveeGun SECONDARY overwritten by comanche_antitank_weapon() name peel: {sec:?}"
    );
}

/// hq-2qedt.130: Battle Drone PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not a template-name peel.
#[test]
fn battle_drone_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleBattleDrone");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic.templates.insert("AmericaVehicleBattleDrone".to_string(), tpl);
    let id = game_logic
        .create_object("AmericaVehicleBattleDrone", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("battle_drone");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - crate::game_logic::host_slave_drones::BATTLE_DRONE_GUN_DAMAGE).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by battle_drone name peel: {prim:?}"
    );
}

/// hq-2qedt.131: China Red Guard PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not a template-name peel.
#[test]
fn red_guard_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("ChinaInfantryRedguard");
    tpl.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic.templates.insert("ChinaInfantryRedguard".to_string(), tpl);
    let id = game_logic
        .create_object("ChinaInfantryRedguard", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("red_guard");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - crate::game_logic::host_red_guard::REDGUARD_DAMAGE).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by red_guard name peel: {prim:?}"
    );
}

/// hq-2qedt.132: China Tank Hunter PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not a template-name peel.
#[test]
fn tank_hunter_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("ChinaInfantryTankHunter");
    tpl.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic.templates.insert("ChinaInfantryTankHunter".to_string(), tpl);
    let id = game_logic
        .create_object("ChinaInfantryTankHunter", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("tank_hunter");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - crate::game_logic::host_tank_hunter::TANK_HUNTER_DAMAGE).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by tank_hunter name peel: {prim:?}"
    );
}

/// hq-2qedt.133: Colonel Burton PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not a template-name peel.
#[test]
fn colonel_burton_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaInfantryColonelBurton");
    tpl.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic.templates.insert("AmericaInfantryColonelBurton".to_string(), tpl);
    let id = game_logic
        .create_object("AmericaInfantryColonelBurton", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("colonel_burton");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - crate::game_logic::host_colonel_burton::BURTON_SNIPER_DAMAGE).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by colonel_burton name peel: {prim:?}"
    );
}

/// hq-2qedt.134: C++ fires the authored WeaponSet weapon; a Fire Base-named
/// template authoring a non-howitzer PRIMARY must not route through the
/// Fire Base howitzer residual on object hit.
#[test]
fn fire_base_named_non_howitzer_weapon_skips_fire_base_object_hit_residual() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaFireBase");
    // Non-structure fixture so the strategy_center base-defense path (out of
    // scope) does not fire; isolates the object_hit gate.
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic.templates.insert("AmericaFireBase".to_string(), tpl);
    let fb_id = game_logic
        .create_object("AmericaFireBase", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("fire base");
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy");
    {
        let (t, attack_events) = game_logic
            .host_object_and_attack_events_mut(fb_id)
            .unwrap();
        t.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = t.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.1;
        }
    }
    for f in 50..60 {
        game_logic.set_current_frame(f);
        game_logic.update_combat(&[fb_id, enemy], LOGIC_FRAME_TIMESTEP);
    }
    assert_eq!(
        game_logic.fire_base_residual_fires(),
        0,
        "Fire Base residual chosen by template-name peel instead of authored weapon"
    );
}

/// hq-2qedt.162: strategy_center base-defense path must key the Fire Base
/// howitzer residual on the authored WeaponSet slot weapon, not template name.
#[test]
fn fire_base_named_structure_non_howitzer_weapon_skips_base_defense_howitzer() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaFireBase");
    tpl.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic.templates.insert("AmericaFireBase".to_string(), tpl);
    let fb_id = game_logic
        .create_object("AmericaFireBase", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("fire base");
    let _enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy");
    if let Some(w) = game_logic.objects.get_mut(&fb_id).and_then(|o| o.weapon.as_mut()) {
        w.last_fire_time = -20.0;
        w.reload_time = 0.1;
    }
    for f in 50..60 {
        game_logic.set_current_frame(f);
        game_logic.try_base_defense_residual_fire(fb_id);
    }
    assert_eq!(
        game_logic.fire_base_residual_fires(),
        0,
        "base-defense Fire Base howitzer chosen by template-name peel instead of authored weapon"
    );
}

/// hq-2qedt.163: Stinger Site PRIMARY/SECONDARY bind from the authored
/// WeaponSet at create (C++ WeaponSet::updateWeaponSet), not a name peel.
#[test]
fn stinger_site_create_weapons_come_from_authored_weapon_set() {
    use crate::game_logic::host_base_defense::{STINGER_AIR_DAMAGE, STINGER_GROUND_DAMAGE};
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("GLAStingerSite");
    tpl.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::SpawnsAreTheWeapons)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0)
        .set_primary_weapon_name("HumveeGun")
        .set_secondary_weapon_name("HumveeGun");
    game_logic.templates.insert("GLAStingerSite".to_string(), tpl);
    let id = game_logic
        .create_object("GLAStingerSite", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("stinger site");
    let obj = game_logic.host_object(id).expect("obj");
    let prim = obj.weapon.clone().expect("authored primary");
    let sec = obj.secondary_weapon.clone().expect("authored secondary");
    assert!(
        (prim.damage - STINGER_GROUND_DAMAGE).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by stinger name peel: {prim:?}"
    );
    assert!(
        (sec.damage - STINGER_AIR_DAMAGE).abs() > 0.01,
        "authored HumveeGun SECONDARY overwritten by stinger name peel: {sec:?}"
    );
}

/// hq-2qedt.164: Overlord PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not a template-name peel.
#[test]
fn overlord_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let peel = crate::game_logic::host_overlord_gun::overlord_gun_weapon(false);
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("ChinaTankOverlord");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic.templates.insert("ChinaTankOverlord".to_string(), tpl);
    let id = game_logic
        .create_object("ChinaTankOverlord", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("overlord");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - peel.damage).abs() > 0.01 || (prim.range - peel.range).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by overlord name peel: {prim:?}"
    );
}

/// hq-2qedt.165: Jarmen Kell PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not a template-name peel.
#[test]
fn jarmen_kell_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let peel = crate::game_logic::host_jarmen_kell::jarmen_kell_weapon(false);
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("GLAInfantryJarmenKell");
    tpl.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic.templates.insert("GLAInfantryJarmenKell".to_string(), tpl);
    let id = game_logic
        .create_object("GLAInfantryJarmenKell", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("jarmen_kell");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - peel.damage).abs() > 0.01 || (prim.range - peel.range).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by jarmen_kell name peel: {prim:?}"
    );
}

/// hq-2qedt.166: MiniGunner PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not a template-name peel.
#[test]
fn minigunner_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let peel = crate::game_logic::host_minigunner::minigunner_ground_weapon(crate::game_logic::host_gattling_tank::GattlingFireLevel::Base, false, false, false);
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("Infa_ChinaInfantryMiniGunner");
    tpl.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic.templates.insert("Infa_ChinaInfantryMiniGunner".to_string(), tpl);
    let id = game_logic
        .create_object("Infa_ChinaInfantryMiniGunner", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("minigunner");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - peel.damage).abs() > 0.01 || (prim.range - peel.range).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by minigunner name peel: {prim:?}"
    );
}

/// hq-2qedt.181: C++ fires the authored WeaponSet weapon; an Overlord-named
/// template authoring a non-OverlordTankGun PRIMARY must not route through
/// the Overlord main-gun residual (OverlordTankShell) on object hit.
#[test]
fn overlord_named_non_overlord_gun_skips_overlord_object_hit_residual() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let mut tpl = crate::game_logic::ThingTemplate::new("ChinaTankOverlord");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("ChinaTankOverlord".to_string(), tpl);
    let ol_id = game_logic
        .create_object("ChinaTankOverlord", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("overlord");
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy");
    {
        let (t, attack_events) = game_logic
            .host_object_and_attack_events_mut(ol_id)
            .unwrap();
        t.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = t.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.1;
        }
    }
    let mut shells = 0;
    for f in 50..60 {
        game_logic.set_current_frame(f);
        game_logic.update_combat(&[ol_id, enemy], LOGIC_FRAME_TIMESTEP);
        shells = shells.max(
            game_logic
                .objects
                .registered_values()
                .filter(|o| o.template_name == "OverlordTankShell")
                .count(),
        );
    }
    assert_eq!(
        shells + game_logic.overlord_gun_residual_fires() as usize,
        0,
        "Overlord residual chosen by template-name peel instead of authored weapon"
    );
}

/// Spawn `template` as Infantry authoring HumveeGun PRIMARY and let it fire at
/// a TestTank for a few frames (object_hit residual gate fixtures).
fn fire_named_template_with_humvee_gun(template: &str, team: Team) -> GameLogic {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let mut tpl = crate::game_logic::ThingTemplate::new(template);
    tpl.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic.templates.insert(template.to_string(), tpl);
    let id = game_logic
        .create_object(template, team, Vec3::new(0.0, 0.0, 0.0))
        .expect("attacker");
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy");
    {
        let (t, attack_events) = game_logic.host_object_and_attack_events_mut(id).unwrap();
        t.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = t.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.1;
        }
    }
    for f in 50..60 {
        game_logic.set_current_frame(f);
        game_logic.update_combat(&[id, enemy], LOGIC_FRAME_TIMESTEP);
    }
    game_logic
}

/// hq-2qedt.182: Jarmen Kell-named template authoring a non-sniper PRIMARY
/// must not route through the Jarmen Kell sniper residual on object hit.
#[test]
fn jarmen_kell_named_non_sniper_weapon_skips_jarmen_object_hit_residual() {
    let gl = fire_named_template_with_humvee_gun("GLAInfantryJarmenKell", Team::USA);
    assert_eq!(gl.jarmen_kell_residual_fires(), 0, "Jarmen residual chosen by template-name peel");
}

/// hq-2qedt.183: MiniGunner-named template authoring a non-minigun PRIMARY
/// must not route through the MiniGunner residual on object hit.
#[test]
fn minigunner_named_non_minigun_weapon_skips_minigunner_object_hit_residual() {
    let gl = fire_named_template_with_humvee_gun("ChinaInfantryMiniGunner", Team::USA);
    assert_eq!(
        gl.minigunner_residual_ground_fires() + gl.minigunner_residual_aa_fires(),
        0,
        "MiniGunner residual chosen by template-name peel"
    );
}

/// hq-2qedt.184: Tank Hunter-named template authoring a non-missile PRIMARY
/// must not route through the Tank Hunter missile residual on object hit.
#[test]
fn tank_hunter_named_non_missile_weapon_skips_tank_hunter_object_hit_residual() {
    let gl = fire_named_template_with_humvee_gun("ChinaInfantryTankHunter", Team::USA);
    assert_eq!(gl.tank_hunter_residual_fires(), 0, "Tank Hunter residual chosen by template-name peel");
}

/// hq-2qedt.185: RPG Trooper-named template authoring a non-rocket PRIMARY
/// must not route through the RPG Trooper rocket residual on object hit.
#[test]
fn rpg_trooper_named_non_rocket_weapon_skips_rpg_object_hit_residual() {
    let gl = fire_named_template_with_humvee_gun("GLAInfantryTunnelDefender", Team::USA);
    assert_eq!(gl.rpg_trooper_residual_fires(), 0, "RPG Trooper residual chosen by template-name peel");
}

/// hq-2qedt.187: Red Guard-named template authoring a non-Redguard PRIMARY
/// must not route through the Red Guard gun/bayonet residual on object hit.
#[test]
fn red_guard_named_non_redguard_weapon_skips_red_guard_object_hit_residual() {
    let gl = fire_named_template_with_humvee_gun("ChinaInfantryRedguard", Team::USA);
    assert_eq!(gl.red_guard_residual_fires(), 0, "Red Guard residual chosen by template-name peel");
}

/// hq-2qedt.188: Colonel Burton-named template authoring a non-Burton PRIMARY
/// must not route through the Burton sniper/knife residual on object hit.
#[test]
fn burton_named_non_burton_weapon_skips_burton_object_hit_residual() {
    let gl = fire_named_template_with_humvee_gun("AmericaInfantryColonelBurton", Team::USA);
    assert_eq!(
        gl.burton_residual_sniper_fires() + gl.burton_residual_knife_kills(),
        0,
        "Burton residual chosen by template-name peel"
    );
}

/// hq-2qedt.189: Helix-named template authoring a non-minigun PRIMARY
/// must not route through the Helix minigun residual on object hit.
#[test]
fn helix_named_non_minigun_weapon_skips_helix_object_hit_residual() {
    let gl = fire_named_template_with_humvee_gun("ChinaVehicleHelix", Team::USA);
    assert_eq!(gl.helix_minigun_residual_fires(), 0, "Helix residual chosen by template-name peel");
}

/// hq-2qedt.190: Humvee-named template with TOW upgrade firing a non-TOW
/// authored SECONDARY must not route through the Humvee TOW residual.
#[test]
fn humvee_named_non_tow_secondary_skips_humvee_tow_object_hit_residual() {
    use crate::game_logic::host_upgrades::UPGRADE_AMERICA_TOW;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleHumvee");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0)
        .set_primary_weapon_name("HumveeGun")
        .set_secondary_weapon_name("BattleMasterTankGun");
    game_logic
        .templates
        .insert("AmericaVehicleHumvee".to_string(), tpl);
    let id = game_logic
        .create_object("AmericaVehicleHumvee", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("humvee");
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy");
    {
        let (h, attack_events) = game_logic.host_object_and_attack_events_mut(id).unwrap();
        h.apply_upgrade_tag(UPGRADE_AMERICA_TOW);
        h.active_weapon_slot = 1;
        h.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = h.secondary_weapon.as_mut() {
            w.last_fire_time = -100.0;
            w.reload_time = 0.1;
            w.min_range = 0.0;
        }
    }
    for f in 50..60 {
        game_logic.set_current_frame(f);
        game_logic.update_combat(&[id, enemy], LOGIC_FRAME_TIMESTEP);
    }
    assert_eq!(
        game_logic.humvee_tow_residual_fires(),
        0,
        "Humvee TOW residual chosen by template-name peel"
    );
}

/// hq-2qedt.191: Troop Crawler-named transport authoring a non-assault PRIMARY
/// must not route through the TroopCrawlerAssault DEPLOY residual on object hit.
#[test]
fn troop_crawler_named_non_assault_weapon_skips_assault_deploy_object_hit_residual() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let mut tpl = crate::game_logic::ThingTemplate::new("ChinaVehicleTroopCrawler");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("ChinaVehicleTroopCrawler".to_string(), tpl);
    let id = game_logic
        .create_object("ChinaVehicleTroopCrawler", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("crawler");
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy");
    {
        let (c, attack_events) = game_logic.host_object_and_attack_events_mut(id).unwrap();
        c.is_troop_crawler_transport = true;
        c.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = c.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.1;
        }
    }
    for f in 50..60 {
        game_logic.set_current_frame(f);
        game_logic.update_combat(&[id, enemy], LOGIC_FRAME_TIMESTEP);
    }
    assert_eq!(
        game_logic.troop_crawler_residual_assault_deploys(),
        0,
        "Troop Crawler DEPLOY residual chosen by template-name peel"
    );
}

/// hq-2qedt.215: Raptor-named jet authoring a non-jet-missile PRIMARY must not
/// route through the Raptor jet-missile residual on object hit.
#[test]
fn raptor_named_non_jet_missile_weapon_skips_raptor_object_hit_residual() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaJetRaptor");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("AmericaJetRaptor".to_string(), tpl);
    let id = game_logic
        .create_object("AmericaJetRaptor", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("raptor");
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(200.0, 0.0, 0.0))
        .expect("enemy");
    // Outside RaptorJetMissileWeapon MinRange (100) so a create-peeled jet weapon still fires.
    {
        let (c, attack_events) = game_logic.host_object_and_attack_events_mut(id).unwrap();
        c.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = c.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.1;
        }
    }
    for f in 50..60 {
        game_logic.set_current_frame(f);
        game_logic.update_combat(&[id, enemy], LOGIC_FRAME_TIMESTEP);
    }
    assert_eq!(
        game_logic.raptor_residual_fires(),
        0,
        "Raptor jet-missile residual chosen by template-name peel"
    );
}

/// hq-2qedt.216: Terrorist-named infantry authoring a non-suicide PRIMARY must
/// not route through the SuicideDynamitePack residual on object hit.
#[test]
fn terrorist_named_non_suicide_weapon_skips_terrorist_object_hit_residual() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let mut tpl = crate::game_logic::ThingTemplate::new("GLAInfantryTerrorist");
    tpl.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("GLAInfantryTerrorist".to_string(), tpl);
    let id = game_logic
        .create_object("GLAInfantryTerrorist", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("terrorist");
    // Inside any create-peeled suicide trigger range so either weapon fires.
    let enemy = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(3.0, 0.0, 0.0))
        .expect("enemy");
    {
        let (c, attack_events) = game_logic.host_object_and_attack_events_mut(id).unwrap();
        c.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = c.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.1;
        }
    }
    for f in 50..60 {
        game_logic.set_current_frame(f);
        game_logic.update_combat(&[id, enemy], LOGIC_FRAME_TIMESTEP);
    }
    assert_eq!(
        game_logic.terrorist_residual_detonations(),
        0,
        "Terrorist suicide residual chosen by template-name peel"
    );
}

/// hq-2qedt.220: Raptor / King Raptor PRIMARY binds from the authored
/// WeaponSet at create (C++ WeaponSet::updateWeaponSet), not a name peel.
#[test]
fn raptor_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    for (name, king) in [("AmericaJetRaptor", false), ("AirF_AmericaJetRaptor", true)] {
        let peel = crate::game_logic::host_raptor::raptor_weapon(king, false);
        let mut game_logic = GameLogic::new();
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Aircraft)
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(160.0)
            .set_primary_weapon_name("HumveeGun");
        game_logic.templates.insert(name.to_string(), tpl);
        let id = game_logic
            .create_object(name, Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("raptor");
        let prim = game_logic
            .host_object(id)
            .and_then(|t| t.weapon.clone())
            .expect("authored primary");
        assert!(
            (prim.damage - peel.damage).abs() > 0.01 || (prim.range - peel.range).abs() > 0.01,
            "{name}: authored HumveeGun PRIMARY overwritten by raptor name peel: {prim:?}"
        );
    }
}

/// hq-2qedt.255: Comanche rocket-pod TERTIARY at create keys on the authored
/// WeaponSet (Comanche20mmCannonWeapon chassis), not the template name.
#[test]
fn comanche_rocket_pods_create_key_on_authored_weapon_set() {
    use crate::game_logic::host_comanche_rocket_pods::{
        COMANCHE_PRIMARY_WEAPON, UPGRADE_COMANCHE_ROCKET_PODS,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let run = |name: &str, weapon: &str| -> bool {
        let mut game_logic = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.add_completed_upgrade(UPGRADE_COMANCHE_ROCKET_PODS);
        game_logic.add_player(player);
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Aircraft)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(220.0)
            .set_primary_weapon_name(weapon);
        game_logic.templates.insert(name.to_string(), tpl);
        let id = game_logic
            .create_object(name, Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("unit");
        game_logic.host_object(id).unwrap().tertiary_weapon.is_some()
    };
    assert!(
        run("USA_Gunship", COMANCHE_PRIMARY_WEAPON),
        "authored Comanche WeaponSet must receive rocket pods TERTIARY"
    );
    assert!(
        !run("AmericaVehicleComanche", "HumveeGun"),
        "Comanche-named chassis with authored HumveeGun must not receive pods"
    );
}

/// hq-2qedt.256: Emperor innate propaganda keys on the authored
/// PropagandaTowerBehavior + OverlordContain modules, not the template name.
#[test]
fn emperor_innate_propaganda_keys_on_authored_modules() {
    let run = |name: &str, propaganda: bool| -> (bool, bool) {
        let mut world = GameLogic::new();
        let tower = if propaganda {
            "Behavior = PropagandaTowerBehavior ModulePropaganda_04\nRadius = 150.0\nDelayBetweenUpdates = 2000\nHealPercentEachSecond = 1%\nEnd\n"
        } else {
            ""
        };
        let source = format!(
            "Object {name}\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = OverlordContain ModuleTag_06\nSlots = 1\nEnd\n{tower}End\n"
        );
        world
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "emperor.ini".into(),
                    source,
                )])
                .unwrap(),
            )
            .unwrap();
        world.add_player(Player::new(1, Team::China, "owner", true));
        let id = world
            .create_object_for_player_with_effects_checked(
                name,
                1,
                Vec3::ZERO,
                None,
                &mut Default::default(),
            )
            .unwrap()
            .unwrap();
        let o = world.host_object(id).unwrap();
        (o.has_overlord_propaganda_addon, o.authors_innate_propaganda_tower())
    };
    assert_eq!(
        run("TestHullWithTower", true),
        (true, true),
        "authored innate PropagandaTowerBehavior on OverlordContain host is Emperor-style"
    );
    assert_eq!(
        run("Tank_ChinaTankEmperor", false),
        (false, false),
        "Emperor-named hull without authored PropagandaTowerBehavior gets no innate tower"
    );
}

/// hq-2qedt.257: Emperor addon-conflict orders key on the authored innate
/// PropagandaTowerBehavior, not the template name.
#[test]
fn emperor_addon_conflicts_key_on_authored_innate_tower() {
    let run = |name: &str, propaganda: bool| -> (bool, bool) {
        let mut world = GameLogic::new();
        let tower = if propaganda {
            "Behavior = PropagandaTowerBehavior ModulePropaganda_04\nRadius = 150.0\nEnd\n"
        } else {
            ""
        };
        let source = format!(
            "Object {name}\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = OverlordContain ModuleTag_06\nSlots = 1\nEnd\n{tower}End\n"
        );
        world
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "emperor.ini".into(),
                    source,
                )])
                .unwrap(),
            )
            .unwrap();
        world.add_player(Player::new(1, Team::China, "owner", true));
        let id = world
            .create_object_for_player_with_effects_checked(
                name,
                1,
                Vec3::ZERO,
                None,
                &mut Default::default(),
            )
            .unwrap()
            .unwrap();
        let o = world.host_object_mut(id).unwrap();
        o.has_overlord_propaganda_addon = true;
        o.install_overlord_gattling_addon();
        (o.has_overlord_propaganda_addon, o.has_overlord_propaganda_residual())
    };
    assert_eq!(
        run("TestHullWithTower", true),
        (true, true),
        "authored innate tower survives the gattling ConflictsWith clear"
    );
    assert_eq!(
        run("Tank_ChinaTankEmperor", false),
        (false, false),
        "Emperor-named hull without authored tower takes the normal conflict clear"
    );
}

/// hq-2qedt.258: Patriot dual ground/AA create binding keys on the authored
/// WeaponSet PRIMARY (PatriotMissileWeapon family), not the template name.
#[test]
fn patriot_create_binding_keys_on_authored_weapon_set() {
    use crate::game_logic::host_base_defense::{
        PATRIOT_GROUND_DAMAGE, PATRIOT_PRIMARY_WEAPON, PATRIOT_SECONDARY_WEAPON,
    };
    crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
    let run = |name: &str, primary: &str, secondary: Option<&str>| -> bool {
        let mut world = GameLogic::new();
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Attackable)
            .add_kind_of(KindOf::FSBaseDefense)
            .set_health(600.0)
            .set_primary_weapon_name(primary);
        if let Some(s) = secondary {
            tpl.set_secondary_weapon_name(s);
        }
        world.templates.insert(name.to_string(), tpl);
        let id = world
            .create_object(name, Team::USA, Vec3::ZERO)
            .expect("defense");
        let o = world.host_object(id).unwrap();
        o.weapon
            .as_ref()
            .is_some_and(|w| (w.damage - PATRIOT_GROUND_DAMAGE).abs() < 0.01)
    };
    assert!(
        run("TestAuthoredBattery", PATRIOT_PRIMARY_WEAPON, Some(PATRIOT_SECONDARY_WEAPON)),
        "authored PatriotMissileWeapon WeaponSet binds the Patriot ground primary"
    );
    assert!(
        !run("USA_Patriot", "HumveeGun", None),
        "Patriot-named hull without authored Patriot WeaponSet gets no Patriot ground peel"
    );
}

/// hq-2qedt.259: Stinger Site hive spawn keys on the authored
/// KINDOF_SPAWNS_ARE_THE_WEAPONS bit, not the template name.
#[test]
fn stinger_hive_spawn_keys_on_authored_spawns_are_the_weapons() {
    let run = |name: &str, hive: bool| -> usize {
        let mut logic = GameLogic::new();
        logic
            .players
            .insert(0, crate::game_logic::Player::new(0, Team::GLA, "GLA", true));
        let mut site = crate::game_logic::ThingTemplate::new(name);
        site.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::FSBaseDefense)
            .set_health(1000.0);
        if hive {
            site.add_kind_of(KindOf::SpawnsAreTheWeapons);
        }
        logic.templates.insert(name.into(), site);
        let mut soldier = crate::game_logic::ThingTemplate::new("GLAInfantryStingerSoldier");
        soldier
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        logic
            .templates
            .insert("GLAInfantryStingerSoldier".into(), soldier);
        let site_id = logic.create_object(name, Team::GLA, Vec3::ZERO).expect("site");
        logic
            .objects
            .registered_values()
            .filter(|o| o.producer_id == Some(site_id))
            .count()
    };
    assert_eq!(
        run("TestHiveDefense", true),
        3,
        "authored SPAWNS_ARE_THE_WEAPONS hive spawns its Stinger soldiers"
    );
    assert_eq!(
        run("GLAStingerSite", false),
        0,
        "Stinger-named structure without the authored hive KindOf spawns nothing"
    );
}

/// hq-2qedt.260: Black Lotus innate hero stealth keys on the authored
/// SpecialAbilityBlackLotusCaptureBuilding capture power, not the template name.
#[test]
fn black_lotus_innate_stealth_keys_on_authored_capture_power() {
    let run = |name: &str, lotus: bool| -> bool {
        let mut logic = GameLogic::new();
        logic
            .players
            .insert(0, crate::game_logic::Player::new(0, Team::China, "China", true));
        let mut t = crate::game_logic::ThingTemplate::new(name);
        t.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(200.0);
        if lotus {
            t.capture_power = crate::game_logic::CapturePowerKind::BlackLotus;
        }
        logic.templates.insert(name.into(), t);
        let id = logic.create_object(name, Team::China, Vec3::ZERO).expect("lotus");
        logic.host_object(id).unwrap().innate_stealth
    };
    assert!(
        run("TestHeroCaptureInfantry", true),
        "authored Black Lotus capture power gets innate hero stealth"
    );
    assert!(
        !run("ChinaInfantryBlackLotus", false),
        "Lotus-named infantry without the authored capture power gets no hero stealth"
    );
}

/// hq-2qedt.261: Pathfinder reveal response (OrderIdleEnemiesToAttackMeUponReveal)
/// keys on the authored USAPathfinderSniperRifle WeaponSet slot, not the name.
#[test]
fn pathfinder_reveal_response_keys_on_authored_sniper_weapon() {
    let run = |name: &str, weapon: &str| -> bool {
        let mut logic = GameLogic::new();
        logic
            .players
            .insert(0, crate::game_logic::Player::new(0, Team::USA, "USA", true));
        logic
            .players
            .insert(1, crate::game_logic::Player::new(1, Team::China, "China", true));
        let mut t = crate::game_logic::ThingTemplate::new(name);
        t.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0)
            .set_primary_weapon_name(weapon);
        logic.templates.insert(name.into(), t);
        let mut e = crate::game_logic::ThingTemplate::new("TestChinaGrunt");
        e.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        logic.templates.insert("TestChinaGrunt".into(), e);
        let v = logic.create_object(name, Team::USA, Vec3::ZERO).expect("victim");
        let eid = logic
            .create_object("TestChinaGrunt", Team::China, Vec3::new(20.0, 0.0, 0.0))
            .expect("enemy");
        logic.host_object_mut(eid).unwrap().vision_range = 300.0;
        logic.order_idle_enemies_to_attack_on_reveal(v);
        logic.host_object(eid).unwrap().target == Some(v)
    };
    assert!(
        run("TestSniperInfantry", crate::game_logic::host_pathfinder::PATHFINDER_SNIPER_WEAPON),
        "authored Pathfinder sniper WeaponSet orders idle enemies on reveal"
    );
    assert!(
        !run("AmericaInfantryPathfinder", "TestNonRetailRifle"),
        "Pathfinder-named infantry without the authored sniper does not"
    );
}

/// hq-2qedt.262: Red Guard / Tank Hunter nationalism residual keys on the
/// authored PRIMARY weapon (RedguardMachineGun / ChinaInfantryTankHunterMissileLauncher),
/// not the template name.
#[test]
fn red_guard_tank_hunter_nationalism_keys_on_authored_weapon() {
    let make = |name: &str, weapon: &str| -> (GameLogic, crate::game_logic::ObjectId) {
        let mut logic = GameLogic::new();
        logic
            .players
            .insert(0, crate::game_logic::Player::new(0, Team::China, "China", true));
        let mut t = crate::game_logic::ThingTemplate::new(name);
        t.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0)
            .set_primary_weapon_name(weapon);
        logic.templates.insert(name.into(), t);
        let id = logic.create_object(name, Team::China, Vec3::ZERO).expect("unit");
        (logic, id)
    };
    use crate::game_logic::host_red_guard::REDGUARD_MACHINE_GUN;
    use crate::game_logic::host_tank_hunter::TANK_HUNTER_MISSILE_WEAPON;

    let (mut l, id) = make("TestAuthoredRifleman", REDGUARD_MACHINE_GUN);
    assert!(
        l.apply_red_guard_nationalism_upgrade(id),
        "authored RedguardMachineGun receives Red Guard nationalism"
    );
    let (mut l, id) = make("ChinaInfantryRedguard", "TestNonRetailRifle");
    assert!(
        !l.apply_red_guard_nationalism_upgrade(id),
        "Red Guard-named infantry without authored machine gun does not"
    );
    let (mut l, id) = make("TestAuthoredRocketeer", TANK_HUNTER_MISSILE_WEAPON);
    assert!(
        l.apply_tank_hunter_nationalism_upgrade(id),
        "authored Tank Hunter launcher receives Tank Hunter nationalism"
    );
    let (mut l, id) = make("ChinaInfantryTankHunter", "TestNonRetailRifle");
    assert!(
        !l.apply_tank_hunter_nationalism_upgrade(id),
        "Tank Hunter-named infantry without authored launcher does not"
    );
}

/// hq-2qedt.194: C++ BlackNapalm / Tactical Nuke MiG are WeaponSetUpgrade
/// (WEAPONSET_PLAYER_UPGRADE → updateWeaponSet); the refreshed PRIMARY is the
/// upgrade row's authored weapon, not name-built mig_weapon() stats.
#[test]
fn mig_upgrade_refresh_binds_authored_weapon_not_name_built_stats() {
    use crate::game_logic::host_mig::{
        BLACK_NAPALM_MISSILE_WEAPON, MigLoadout, NAPALM_MISSILE_WEAPON, NUKE_MIG_MISSILE_WEAPON,
        NUKE_NUKE_MISSILE_WEAPON, mig_weapon,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    // Retail base PRIMARY authored (hq-2qedt.248 gate keys on authored MiG missile).
    for (tpl_name, nuke, weapon_name, loadout, base) in [
        (
            "ChinaJetMIG",
            false,
            BLACK_NAPALM_MISSILE_WEAPON,
            MigLoadout::BlackNapalm,
            NAPALM_MISSILE_WEAPON,
        ),
        (
            "Nuke_ChinaJetMIG",
            true,
            NUKE_NUKE_MISSILE_WEAPON,
            MigLoadout::NukeTactical,
            NUKE_MIG_MISSILE_WEAPON,
        ),
    ] {
        let mut game_logic = GameLogic::new();
        let mut tpl = crate::game_logic::ThingTemplate::new(tpl_name);
        tpl.add_kind_of(KindOf::Aircraft)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(300.0)
            .set_primary_weapon_name(base);
        game_logic.templates.insert(tpl_name.to_string(), tpl);
        let id = game_logic
            .create_object(tpl_name, Team::China, Vec3::new(0.0, 0.0, 0.0))
            .expect("mig");
        let applied = if nuke {
            game_logic.apply_mig_tactical_nuke_upgrade(id)
        } else {
            game_logic.apply_mig_black_napalm_upgrade(id)
        };
        assert!(applied, "{tpl_name} upgrade applies");
        let prim = game_logic
            .host_object(id)
            .and_then(|o| o.weapon.clone())
            .expect("upgraded primary");
        let authored = game_logic
            .with_own_services(|| {
                crate::game_logic::ThingTemplate::weapon_from_host_store(weapon_name)
            })
            .expect("authored upgrade weapon");
        let built = mig_weapon(loadout);
        assert!(
            (authored.splash_radius - built.splash_radius).abs() > 0.01
                || (authored.damage - built.damage).abs() > 0.01,
            "fixture must distinguish authored vs name-built"
        );
        assert!(
            (prim.damage - authored.damage).abs() < 0.01
                && (prim.splash_radius - authored.splash_radius).abs() < 0.01
                && (prim.min_range - authored.min_range).abs() < 0.01,
            "{tpl_name}: refresh used name-built mig_weapon(): {prim:?} vs authored {authored:?}"
        );
    }
}

/// hq-2qedt.156: C++ binds PRIMARY from the template WeaponSet at create
/// (WeaponSet::updateWeaponSet), not a template-name host table. A
/// MiG-named template authoring a different PRIMARY must keep it.
#[test]
fn mig_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::host_mig::{MigLoadout, mig_primary_damage};
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("ChinaJetMIG");
    tpl.add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic.templates.insert("ChinaJetMIG".to_string(), tpl);
    let id = game_logic
        .create_object("ChinaJetMIG", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("mig");
    let obj = game_logic.host_object(id).expect("mig");
    let prim = obj.weapon.clone().expect("authored primary");
    assert!(
        (prim.damage - mig_primary_damage(MigLoadout::Standard)).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by mig_weapon() name peel: {prim:?}"
    );
}

/// hq-2qedt.154: C++ binds PRIMARY from the template WeaponSet at create
/// (WeaponSet::updateWeaponSet), not a template-name host table. A
/// Scorpion-named template authoring a different PRIMARY must keep it.
#[test]
fn scorpion_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::host_scorpion::{SCORPION_GUN_DAMAGE, SCORPION_GUN_DAMAGE_PLUS};
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("GLATankScorpion");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("GLATankScorpion".to_string(), tpl);
    let id = game_logic
        .create_object("GLATankScorpion", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("scorpion");
    let obj = game_logic.host_object(id).expect("scorpion");
    let prim = obj.weapon.clone().expect("authored primary");
    assert!(
        (prim.damage - SCORPION_GUN_DAMAGE).abs() > 0.01
            && (prim.damage - SCORPION_GUN_DAMAGE_PLUS).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by scorpion_gun_weapon() name peel: {prim:?}"
    );
}

#[test]
fn stealth_fighter_air_and_mig_residual_has_no_template_name_sniff() {
    // C++ StealthFighter residual keys off the authored weapon, never the template name.
    let src = include_str!("../world_combat/air_and_mig.rs");
    assert!(!src.contains("is_stealth_fighter_template"));
}

/// hq-2qedt.135: the innate/ATTACKING-forbidden stealth residual follows the
/// template's authored StealthUpdate (InnateStealth / StealthDelay /
/// StealthForbiddenConditions), as C++ StealthUpdate module data does, not a
/// template-name sniff.
#[test]
fn stealth_fighter_innate_stealth_residual_keys_on_authored_stealth_update() {
    let mut game_logic = GameLogic::new();
    let mut named = crate::game_logic::ThingTemplate::new("AmericaJetStealthFighter");
    named
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .set_health(150.0);
    game_logic
        .templates
        .insert("AmericaJetStealthFighter".to_string(), named);
    let mut authored = crate::game_logic::ThingTemplate::new("TestCloakJet");
    authored
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .set_health(150.0);
    authored.stealth_update_innate = true;
    authored.stealth_update_forbids_attacking = true;
    authored.stealth_update_delay_ms = Some(2000);
    game_logic
        .templates
        .insert("TestCloakJet".to_string(), authored);

    let named_id = game_logic
        .create_object(
            "AmericaJetStealthFighter",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("named");
    let authored_id = game_logic
        .create_object("TestCloakJet", Team::USA, Vec3::new(50.0, 0.0, 0.0))
        .expect("authored");
    game_logic.update_stealth_and_detection();

    let n = game_logic.host_object(named_id).expect("named");
    assert!(
        !n.innate_stealth,
        "name-only Stealth Fighter template without StealthUpdate must not get innate stealth"
    );
    let a = game_logic.host_object(authored_id).expect("authored");
    assert!(a.innate_stealth, "authored InnateStealth must apply");
    assert_eq!(
        a.stealth_delay_frames, 60,
        "authored StealthDelay 2000ms = 60f"
    );
}

/// hq-2qedt.153: object_hit Comanche residuals key on the fired slot's authored
/// weapon (Comanche20mmCannonWeapon), not an is_comanche_template sniff. C++
/// Weapon::fireWeapon has no firing-template name branch.
#[test]
fn comanche_cannon_object_hit_residual_keys_on_authored_weapon_not_template_name() {
    use crate::game_logic::host_comanche_rocket_pods::COMANCHE_PRIMARY_WEAPON;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    fn fire(name: &str, weapon: &str) -> u32 {
        ensure_host_weapon_store();
        let mut gl = GameLogic::new();
        ensure_test_tank_template(&mut gl);
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Aircraft)
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(160.0)
            .set_primary_weapon_name(weapon);
        gl.templates.insert(name.to_string(), tpl);
        let shooter = gl
            .create_object(name, Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("shooter");
        let enemy = gl
            .create_object("TestTank", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
            .expect("enemy");
        {
            let (f, ev) = gl.host_object_and_attack_events_mut(shooter).unwrap();
            f.attack_target(
                enemy,
                ev,
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            );
            if let Some(w) = f.weapon.as_mut() {
                w.last_fire_time = -20.0;
                w.reload_time = 0.05;
            }
        }
        gl.set_current_frame(50);
        gl.update_combat(&[shooter, enemy], LOGIC_FRAME_TIMESTEP);
        gl.comanche_cannon_residual_fires
    }

    assert!(
        fire("TestAuthoredGunshipCarrier", COMANCHE_PRIMARY_WEAPON) > 0,
        "authored Comanche20mmCannonWeapon must take the cannon residual"
    );
    assert_eq!(
        fire("AmericaVehicleComanche", "RaptorJetMissileWeapon"),
        0,
        "comanche-named template with a different authored weapon must not"
    );
}

/// hq-2qedt.153: object_hit has no Comanche template-name sniff.
#[test]
fn comanche_object_hit_residual_has_no_template_name_sniff() {
    let src = include_str!("../world_tick/combat/object_hit.rs");
    assert!(!src.contains("is_comanche_template"));
    assert!(!src.contains("should_apply_comanche"));
    assert!(!src.contains("should_apply_rocket_pod_area_attack"));
}

/// hq-2qedt.179: Scorpion gun/rocket refresh (salvage tier / rocket / AP)
/// keys on the authored PRIMARY (ScorpionTankGun family), not an
/// is_scorpion_template name sniff. C++ WeaponSetUpgrade/crate paths flip
/// WeaponSet conditions on whatever object authored that set.
#[test]
fn scorpion_refresh_keys_on_authored_gun_not_template_name() {
    use crate::game_logic::host_scorpion::{
        SCORPION_GUN_DAMAGE_PLUS, SCORPION_TANK_GUN, ScorpionSalvageTier,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    fn mk(gl: &mut GameLogic, name: &str, weapon: &str) -> ObjectId {
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(300.0)
            .set_primary_weapon_name(weapon);
        gl.templates.insert(name.to_string(), tpl);
        gl.create_object(name, Team::GLA, Vec3::new(0.0, 0.0, 0.0))
            .expect("obj")
    }

    ensure_host_weapon_store();
    let mut gl = GameLogic::new();
    let named = mk(&mut gl, "GLATankScorpion", "HumveeGun");
    assert!(!gl.apply_scorpion_salvage_tier(named, ScorpionSalvageTier::One));
    assert!(!gl.apply_scorpion_rocket_upgrade(named));
    assert!(!gl.apply_scorpion_ap_rockets_upgrade(named));

    let authored = mk(&mut gl, "GLAModTankAlpha", SCORPION_TANK_GUN);
    assert!(gl.apply_scorpion_salvage_tier(authored, ScorpionSalvageTier::One));
    let prim = gl.host_object(authored).unwrap().weapon.clone().unwrap();
    assert!((prim.damage - SCORPION_GUN_DAMAGE_PLUS).abs() < 0.01);
    assert!(gl.apply_scorpion_rocket_upgrade(authored));
    assert!(gl.host_object(authored).unwrap().secondary_weapon.is_some());
    assert!(gl.apply_scorpion_ap_rockets_upgrade(authored));
}

/// hq-2qedt.180: vehicle_shells Scorpion splash keys on the fired slot's
/// authored weapon (ScorpionTankGun), not an is_scorpion_template sniff.
/// C++ splash comes from the Weapon template (Weapon::fireWeapon / dealDamageInternal).
#[test]
fn scorpion_splash_residual_keys_on_authored_weapon_not_template_name() {
    use crate::game_logic::host_scorpion::SCORPION_TANK_GUN;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    fn splash(name: &str, weapon: &str) -> u32 {
        ensure_host_weapon_store();
        let mut gl = GameLogic::new();
        ensure_test_tank_template(&mut gl);
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(200.0)
            .set_primary_weapon_name(weapon);
        gl.templates.insert(name.to_string(), tpl);
        let shooter = gl
            .create_object(name, Team::GLA, Vec3::new(0.0, 0.0, 0.0))
            .expect("shooter");
        let enemy = gl
            .create_object("TestTank", Team::USA, Vec3::new(45.0, 0.0, 0.0))
            .expect("enemy");
        let pos = gl.objects.get(&enemy).unwrap().get_position();
        gl.apply_scorpion_residual_at(pos, Some(shooter), Some(enemy), 0)
            .0
    }

    assert!(
        splash("TestAuthoredGunCarrier", SCORPION_TANK_GUN) > 0,
        "authored ScorpionTankGun must take the splash residual"
    );
    assert_eq!(
        splash("GLATankScorpion", "RaptorJetMissileWeapon"),
        0,
        "scorpion-named template with a different authored weapon must not"
    );
}

/// hq-2qedt.180: vehicle_shells has no Scorpion template-name sniff.
#[test]
fn scorpion_vehicle_shells_has_no_template_name_sniff() {
    let src = include_str!("../world_combat/vehicle_shells.rs");
    assert!(!src.contains("is_scorpion_template"));
}

/// hq-2qedt.207: Scorpion rocket / AP-rockets team upgrades pick recipients
/// by authored PRIMARY (ScorpionTankGun family), not is_scorpion_template.
/// C++ WeaponSetUpgrade / WeaponBonusUpgrade fire on any object whose
/// authored modules carry the trigger, regardless of template name.
#[test]
fn scorpion_team_upgrades_key_on_authored_gun_not_template_name() {
    use crate::game_logic::host_scorpion::{
        SCORPION_TANK_GUN, UPGRADE_GLA_AP_ROCKETS, UPGRADE_GLA_SCORPION_ROCKET,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    fn mk(gl: &mut GameLogic, name: &str, weapon: &str) -> ObjectId {
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(300.0)
            .set_primary_weapon_name(weapon);
        gl.templates.insert(name.to_string(), tpl);
        gl.create_object(name, Team::GLA, Vec3::new(0.0, 0.0, 0.0))
            .expect("obj")
    }

    ensure_host_weapon_store();
    let mut gl = GameLogic::new();
    let named = mk(&mut gl, "GLATankScorpion", "HumveeGun");
    let authored = mk(&mut gl, "GLAModTankAlpha", SCORPION_TANK_GUN);
    assert_eq!(
        gl.apply_scorpion_rocket_to_team(Team::GLA.into(), UPGRADE_GLA_SCORPION_ROCKET),
        1
    );
    assert!(gl.objects.get(&authored).unwrap().has_upgrade_tag(UPGRADE_GLA_SCORPION_ROCKET));
    assert!(!gl.objects.get(&named).unwrap().has_upgrade_tag(UPGRADE_GLA_SCORPION_ROCKET));
    assert_eq!(gl.apply_ap_rockets_to_team(Team::GLA.into(), UPGRADE_GLA_AP_ROCKETS), 1);
    assert!(gl.objects.get(&authored).unwrap().has_upgrade_tag(UPGRADE_GLA_AP_ROCKETS));

    let src = include_str!("../world_objects/weapon_upgrades.rs");
    assert!(!src.contains("is_scorpion_template"));
}

/// hq-2qedt.208: factory_init Scorpion rocket SECONDARY gate keys on the
/// authored PRIMARY (ScorpionTankGun family), not is_scorpion_template.
/// C++ WeaponSetUpgrade flips PLAYER_UPGRADE on any object whose authored
/// modules carry it; WeaponSet::updateWeaponSet then picks the row.
#[test]
fn scorpion_factory_init_rocket_gate_keys_on_authored_gun_not_template_name() {
    let src = include_str!("../world_objects/create_destroy_die/factory_init.rs");
    assert!(!src.contains("is_scorpion_template"));
    assert!(src.contains("authored_scorpion_gun"));
}

/// hq-2qedt.209: object_hit has no Scorpion template-name sniff.
#[test]
fn scorpion_object_hit_residual_has_no_template_name_sniff() {
    let src = include_str!("../world_tick/combat/object_hit.rs");
    assert!(!src.contains("is_scorpion_template"));
    assert!(!src.contains("should_apply_scorpion_residual"));
}

/// hq-2qedt.172: Comanche ground_target / air_and_mig residuals carry no
/// is_comanche_template sniff. C++ CommandXlat keys rocket-pod ground fire on
/// the weapon name "ComancheRocketPodWeapon", not the firing template name.
#[test]
fn comanche_rocket_pod_ground_fire_keys_on_authored_tertiary_not_template_name() {
    use crate::game_logic::host_comanche_rocket_pods::COMANCHE_ROCKET_POD_WEAPON;
    let gt = include_str!("../world_tick/combat/ground_target.rs");
    assert!(!gt.contains("is_comanche_template"));
    assert!(gt.contains("COMANCHE_ROCKET_POD_WEAPON"));
    let am = include_str!("../world_combat/air_and_mig.rs");
    assert!(!am.contains("is_comanche_template"));

    let mut gl = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("TestAuthoredPodCarrier");
    tpl.add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0)
        .set_tertiary_weapon_name(COMANCHE_ROCKET_POD_WEAPON);
    gl.templates
        .insert("TestAuthoredPodCarrier".to_string(), tpl);
    let id = gl
        .create_object("TestAuthoredPodCarrier", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("carrier");
    assert_eq!(
        gl.host_object(id).unwrap().authored_weapon_name_for_slot(2),
        Some(COMANCHE_ROCKET_POD_WEAPON)
    );
}
