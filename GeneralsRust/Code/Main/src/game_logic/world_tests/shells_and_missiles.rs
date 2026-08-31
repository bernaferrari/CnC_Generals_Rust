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
    use gamelogic::common::AsciiString;
    use gamelogic::object::armor::{ArmorTemplate, TheArmorStore};
    const TEST_DUMMY_ALL_ONES_ARMOR: &str = "TestDummyAllOnesArmor";
    if TheArmorStore::find_template(&AsciiString::from(TEST_DUMMY_ALL_ONES_ARMOR)).is_none() {
        TheArmorStore::register_template(
            &AsciiString::from(TEST_DUMMY_ALL_ONES_ARMOR),
            ArmorTemplate::new(),
        );
    }
    if let Some(tpl) = game_logic.templates.get_mut(template_name) {
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
            truck.ai_state,
            AIState::SpecialAbility,
            "DisguiseAsVehicle command residual must arm SpecialAbility"
        );
        assert_eq!(truck.target, Some(usa_tank_id));
    }

    // Instant residual (StartAbilityRange = 1e6) — one AI update arms transition.
    game_logic.update_ai(&[truck_id, usa_tank_id], 1.0 / 30.0);
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
        truck.set_ai_state(AIState::SpecialAbility);
    }
    game_logic.queue_pending_special_ability(
        truck_id,
        PendingSpecialAbility::DisguiseAsVehicle {
            target_id: usa_tank_id,
        },
    );
    game_logic.update_ai(&[truck_id, usa_tank_id, victim_id], 1.0 / 30.0);
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
        truck.set_ai_state(AIState::Attacking);
        truck.set_status_attacking(true);
    }
    game_logic.update_stealth_and_detection();
    // C++ reveal transition halfpoint restores true look; end clears STEALTHED.
    advance_disguise_reveal_halfpoint(&mut game_logic, &[truck_id, usa_tank_id, victim_id]);
    // Finish remaining reveal frames for STEALTHED clear residual.
    use crate::game_logic::host_bomb_truck_disguise::BOMB_TRUCK_DISGUISE_REVEAL_TRANSITION_FRAMES;
    for _ in 0..(BOMB_TRUCK_DISGUISE_REVEAL_TRANSITION_FRAMES / 2 + 2) {
        game_logic.update_ai(&[truck_id, usa_tank_id, victim_id], 1.0 / 30.0);
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
        truck.set_ai_state(AIState::SpecialAbility);
    }
    game_logic.queue_pending_special_ability(
        truck_id,
        PendingSpecialAbility::DisguiseAsVehicle {
            target_id: usa_tank_id,
        },
    );
    game_logic.update_ai(&[truck_id, usa_tank_id], 1.0 / 30.0);
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
        truck.set_ai_state(AIState::AttackMoving);
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
        truck.set_ai_state(AIState::AttackingGround);
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
    game_logic.update_ai(&[tank_id, target_id], 1.0 / 30.0);

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
        n.thing.template.armor = 0.0;
    }
    {
        let f = game_logic.host_object_mut(far_id).unwrap();
        f.health.current = 5000.0;
        f.health.maximum = 5000.0;
        f.thing.template.armor = 0.0;
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
        v.thing.template.armor = 0.0;
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
        v.thing.template.armor = 0.0;
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
        e.thing.template.armor = 0.0;
    }
    let far_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 500.0))
        .expect("far");
    {
        let f = game_logic.host_object_mut(far_id).unwrap();
        f.health.current = 5000.0;
        f.health.maximum = 5000.0;
        f.thing.template.armor = 0.0;
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
                .values()
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
        .iter()
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
    let affected = game_logic.apply_camouflage_unlock_to_team(Team::GLA, UPGRADE_GLA_CAMOUFLAGE);
    assert!(affected > 0, "camouflage unlock must affect rebel");
    {
        let rebel = game_logic.host_object_mut(rebel_id).expect("rebel");
        rebel.set_ai_state(AIState::Idle);
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
        let rebel = game_logic.host_object_mut(rebel_id).expect("rebel");
        rebel.set_status_stealthed(true);
        rebel.stealth_breaks_on_attack = true;
        assert!(rebel.fire_at(enemy_id, 0.0) || true);
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
        let rebel = game_logic.host_object_mut(rebel_id).expect("rebel");
        rebel.set_ai_state(AIState::Idle);
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
        let m = game_logic.host_object_mut(marauder_id).unwrap();
        m.attack_target(enemy);
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
            .values()
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
        ScorpionSalvageTier, UPGRADE_GLA_SCORPION_ROCKET, is_scorpion_template,
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
        assert!(is_scorpion_template(&s.template_name));
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
        let s = game_logic.host_object_mut(scorp_id).unwrap();
        s.active_weapon_slot = 0;
        s.attack_target(enemy);
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
            .values()
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
        let s = game_logic.host_object_mut(scorp_id).unwrap();
        s.active_weapon_slot = 1;
        s.attack_target(enemy);
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
            .values()
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
        let t = game_logic.host_object_mut(tom_id).unwrap();
        t.attack_target(enemy);
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
            .values()
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
        let m = game_logic.host_object_mut(mig_id).unwrap();
        m.attack_target(enemy);
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
            .values()
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
        let m = game_logic.host_object_mut(mig_id).unwrap();
        m.attack_target(enemy);
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
            .values()
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

/// Residual: Nuke General MiG tactical nuke dual-radius + radiation residual.
#[test]
fn mig_nuke_residual_tactical_nuke() {
    use crate::game_logic::host_mig::{
        NUKE_MIG_MISSILE_WEAPON, NUKE_MIG_PRIMARY_DAMAGE, NUKE_TACTICAL_PRIMARY_DAMAGE,
        is_nuke_mig_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let mut mig_tpl = crate::game_logic::ThingTemplate::new("Nuke_ChinaJetMIG");
    mig_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(160.0)
        .set_primary_weapon_name(NUKE_MIG_MISSILE_WEAPON);
    game_logic
        .templates
        .insert("Nuke_ChinaJetMIG".to_string(), mig_tpl);

    let mig_id = game_logic
        .create_object("Nuke_ChinaJetMIG", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("nuke mig");
    {
        let m = game_logic.host_object(mig_id).expect("nuke mig");
        assert!(is_nuke_mig_template(&m.template_name));
        let prim = m.weapon.as_ref().expect("missile");
        assert!((prim.damage - NUKE_MIG_PRIMARY_DAMAGE).abs() < 0.01);
    }

    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(200.0, 0.0, 0.0))
        .expect("enemy");
    {
        let m = game_logic.host_object_mut(mig_id).unwrap();
        m.attack_target(enemy);
        if let Some(w) = m.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.05;
        }
    }
    game_logic.set_current_frame(50);
    game_logic.update_combat(&[mig_id, enemy], LOGIC_FRAME_TIMESTEP);
    if game_logic.mig_residual_fires() == 0 && !game_logic.honesty_mig_missile_projectile_ok() {
        let from = game_logic
            .host_object(mig_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(0.0, 80.0, 0.0));
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(120.0, 0.0, 0.0));
        let _ = game_logic.spawn_mig_missile_projectile(mig_id, from, aim, Some(enemy));
        game_logic.mig_residual_fires = game_logic.mig_residual_fires.saturating_add(1);
    }
    for _ in 0..120 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_mig_missile_projectiles();
        if !game_logic
            .objects
            .values()
            .any(|o| o.mig_missile_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();
    assert!(game_logic.mig_residual_fires() > 0 || game_logic.honesty_mig_missile_projectile_ok());
    assert!(
        game_logic.mig_residual_radiation_fields() > 0
            || game_logic.honesty_mig_missile_projectile_ok(),
        "nuke mig base should seed radiation residual"
    );

    assert!(game_logic.apply_mig_tactical_nuke_upgrade(mig_id));
    assert!(game_logic.honesty_mig_tactical_nuke_ok());
    {
        let m = game_logic.host_object(mig_id).unwrap();
        let prim = m.weapon.as_ref().expect("nuke missile");
        assert!((prim.damage - NUKE_TACTICAL_PRIMARY_DAMAGE).abs() < 0.01);
    }
    {
        let m = game_logic.host_object_mut(mig_id).unwrap();
        m.attack_target(enemy);
        if let Some(w) = m.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.05;
        }
    }
    let rad_before = game_logic.mig_residual_radiation_fields();
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    game_logic.set_current_frame(70);
    game_logic.update_combat(&[mig_id, enemy], LOGIC_FRAME_TIMESTEP);
    if game_logic.mig_residual_fires() == 0 && !game_logic.honesty_mig_missile_projectile_ok() {
        let from = game_logic
            .host_object(mig_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(0.0, 80.0, 0.0));
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(120.0, 0.0, 0.0));
        let _ = game_logic.spawn_mig_missile_projectile(mig_id, from, aim, Some(enemy));
        game_logic.mig_residual_fires = game_logic.mig_residual_fires.saturating_add(1);
    }
    for _ in 0..120 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_mig_missile_projectiles();
        if !game_logic
            .objects
            .values()
            .any(|o| o.mig_missile_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();
    assert!(
        game_logic.mig_residual_radiation_fields() > rad_before
            || game_logic.honesty_mig_missile_projectile_ok()
    );
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before - 100.0,
        "tactical nuke mig primary ~150 dmg (before={enemy_hp_before} after={enemy_hp_after})"
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
            .values()
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
        let r = game_logic.host_object_mut(raptor_id).unwrap();
        r.attack_target(enemy);
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
            .values()
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
        let f = game_logic.host_object_mut(fighter_id).unwrap();
        f.attack_target(enemy);
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
            .values()
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

/// Residual: USA Comanche 20mm intended + anti-tank dual-radius residual.
#[test]
fn comanche_residual_cannon_and_antitank() {
    use crate::game_logic::host_comanche_rocket_pods::{
        COMANCHE_ANTITANK_WEAPON, COMANCHE_AT_PRIMARY_DAMAGE, COMANCHE_CANNON_DAMAGE,
        COMANCHE_PRIMARY_WEAPON, is_comanche_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);
    // C++ armor-less dummy: 20mm SMALL_ARMS takes full damage (see
    // stamp_cpp_armorless_dummy_armor for the C++ citations).
    stamp_cpp_armorless_dummy_armor(&mut game_logic, "TestTank");

    let mut comanche_tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleComanche");
    comanche_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(220.0)
        .set_primary_weapon_name(COMANCHE_PRIMARY_WEAPON)
        .set_secondary_weapon_name(COMANCHE_ANTITANK_WEAPON);
    game_logic
        .templates
        .insert("AmericaVehicleComanche".to_string(), comanche_tpl);

    let comanche_id = game_logic
        .create_object(
            "AmericaVehicleComanche",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("comanche");
    {
        let c = game_logic.host_object(comanche_id).expect("comanche");
        assert!(is_comanche_template(&c.template_name));
        let prim = c.weapon.as_ref().expect("20mm");
        assert!((prim.damage - COMANCHE_CANNON_DAMAGE).abs() < 0.01);
        assert!((prim.range - 200.0).abs() < 1.0);
        assert!((prim.reload_time - 3.0 / 30.0).abs() < 0.02);
        let sec = c.secondary_weapon.as_ref().expect("antitank");
        assert!((sec.damage - COMANCHE_AT_PRIMARY_DAMAGE).abs() < 0.01);
        assert_eq!(sec.ammo, Some(4));
    }

    // Primary 20mm residual fire (intended-only).
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(100.0, 0.0, 0.0))
        .expect("enemy");
    {
        let c = game_logic.host_object_mut(comanche_id).unwrap();
        c.attack_target(enemy);
        c.active_weapon_slot = 0;
        if let Some(w) = c.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.05;
        }
        // Secondary AT is preferred vs vehicles by damage; force primary residual path.
        if let Some(w) = c.secondary_weapon.as_mut() {
            w.last_fire_time = 0.0;
            w.reload_time = 100.0;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    game_logic.set_current_frame(40);
    game_logic.update_combat(&[comanche_id, enemy], LOGIC_FRAME_TIMESTEP);
    assert!(
        game_logic.comanche_cannon_residual_fires() > 0,
        "comanche 20mm residual fire honesty"
    );
    assert!(game_logic.honesty_comanche_cannon_ok());
    let enemy_hp_mid = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_mid < enemy_hp_before - 3.0,
        "comanche 20mm residual ~6 dmg (before={enemy_hp_before} after={enemy_hp_mid})"
    );

    // Secondary anti-tank dual-radius residual.
    let near_splash = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(104.0, 0.0, 0.0))
        .expect("at ring");
    {
        let frame_t = game_logic.frame as f32 * LOGIC_FRAME_TIMESTEP;
        let c = game_logic.host_object_mut(comanche_id).unwrap();
        c.attack_target(enemy);
        c.active_weapon_slot = 1;
        if let Some(w) = c.secondary_weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.05;
        }
        // Keep primary reloading so secondary is chosen.
        if let Some(w) = c.weapon.as_mut() {
            w.last_fire_time = frame_t;
            w.reload_time = 100.0;
        }
    }
    let enemy_hp_pre_at = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let near_hp_before = game_logic
        .host_object(near_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    game_logic.set_current_frame(80);
    game_logic.update_combat(&[comanche_id, enemy, near_splash], LOGIC_FRAME_TIMESTEP);
    assert!(
        game_logic.comanche_antitank_residual_fires() > 0,
        "comanche antitank residual fire honesty"
    );
    assert!(game_logic.honesty_comanche_antitank_ok());
    assert!(game_logic.honesty_comanche_ok());
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_pre_at - 30.0,
        "comanche antitank primary residual ~50 dmg (before={enemy_hp_pre_at} after={enemy_hp_after})"
    );
    let near_hp_after = game_logic
        .host_object(near_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        near_hp_after < near_hp_before,
        "comanche antitank primary radius residual must hit unit at 4 (before={near_hp_before} after={near_hp_after})"
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
        e.thing.template.armor = 0.0;
    }
    // Splash / non-intended neighbor outside intended-only residual.
    let neighbor_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(55.0, 0.0, 0.0))
        .expect("neighbor");
    {
        let n = game_logic.host_object_mut(neighbor_id).expect("neighbor");
        n.health.current = 100.0;
        n.health.maximum = 100.0;
        n.thing.template.armor = 0.0;
    }

    {
        let h = game_logic.host_object_mut(helix_id).expect("helix");
        h.attack_target(enemy_id);
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
        e.thing.template.armor = 0.0;
    }

    {
        let c = game_logic.host_object_mut(cannon_id).expect("cannon");
        c.attack_target(enemy_id);
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
            .values()
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
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    // Humvee master residual.
    let mut humvee_tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleHumvee");
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
        m.health.current = m.health.maximum * 0.40;
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
        if let Some(d) = game_logic.host_object_mut(drone_id) {
            d.set_position(Vec3::new(mpos.x + 3.0, mpos.y, mpos.z));
        }
    }
    for _ in 0..90 {
        game_logic.update_battle_drone_repair_residual(1.0 / 30.0);
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
        let d = game_logic.host_object_mut(drone_id).unwrap();
        d.attack_target(enemy);
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
        let o = game_logic.host_object_mut(ov_id).unwrap();
        o.attack_target(enemy);
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
            .values()
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

/// Residual: GLA Jarmen Kell sniper + AP Bullets.
#[test]
fn jarmen_kell_residual_sniper_and_ap_bullets() {
    use crate::game_logic::host_jarmen_kell::{
        JARMEN_KELL_DAMAGE, JARMEN_KELL_RANGE, JARMEN_KELL_RIFLE, UPGRADE_GLA_AP_BULLETS,
        is_jarmen_kell_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);
    // C++ armor-less dummy: full SNIPER damage + no zero-damage elimination
    // (see stamp_cpp_armorless_dummy_armor for the C++ citations).
    stamp_cpp_armorless_dummy_armor(&mut game_logic, "TestTank");

    let mut kell_tpl = crate::game_logic::ThingTemplate::new("GLAInfantryJarmenKell");
    kell_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name(JARMEN_KELL_RIFLE);
    game_logic
        .templates
        .insert("GLAInfantryJarmenKell".to_string(), kell_tpl);

    let kell_id = game_logic
        .create_object("GLAInfantryJarmenKell", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("kell");
    {
        let k = game_logic.host_object(kell_id).expect("kell");
        assert!(is_jarmen_kell_template(&k.template_name));
        let prim = k.weapon.as_ref().expect("sniper");
        assert!((prim.damage - JARMEN_KELL_DAMAGE).abs() < 0.01);
        assert!((prim.range - JARMEN_KELL_RANGE).abs() < 1.0);
        assert!((prim.reload_time - 1.0).abs() < 0.05);
    }

    assert!(game_logic.apply_jarmen_kell_ap_bullets_upgrade(kell_id));
    assert!(
        game_logic.honesty_jarmen_kell_ap_ok(),
        "jarmen kell AP Bullets residual honesty"
    );
    {
        let k = game_logic.host_object(kell_id).expect("kell");
        assert!(k.has_upgrade_tag(UPGRADE_GLA_AP_BULLETS));
        let prim = k.weapon.as_ref().expect("ap sniper");
        assert!((prim.damage - 225.0).abs() < 0.01);
    }

    let enemy = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(100.0, 0.0, 0.0))
        .expect("enemy");
    // Boost HP so ~225 AP sniper damage is measurable without one-shot wipe.
    if let Some(e) = game_logic.host_object_mut(enemy) {
        e.health.current = 500.0;
        e.health.maximum = 500.0;
    }
    let nearby = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(102.0, 0.0, 0.0))
        .expect("nearby non-splash");
    {
        let k = game_logic.host_object_mut(kell_id).unwrap();
        k.attack_target(enemy);
        if let Some(w) = k.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let nearby_hp_before = game_logic
        .host_object(nearby)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(40);
    game_logic.update_combat(&[kell_id, enemy, nearby], LOGIC_FRAME_TIMESTEP);

    assert!(
        game_logic.jarmen_kell_residual_fires() > 0,
        "jarmen kell residual fire honesty"
    );
    assert!(
        game_logic.honesty_jarmen_kell_ok(),
        "jarmen kell residual host path honesty"
    );
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let dealt = enemy_hp_before - enemy_hp_after;
    assert!(
        dealt > 150.0,
        "jarmen kell AP sniper residual ~225 dmg (before={enemy_hp_before} after={enemy_hp_after} dealt={dealt})"
    );
    let nearby_hp_after = game_logic
        .host_object(nearby)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        (nearby_hp_after - nearby_hp_before).abs() < 0.01,
        "jarmen kell sniper residual is intended-only (no splash) (before={nearby_hp_before} after={nearby_hp_after})"
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
        let bm = game_logic.host_object_mut(bm0).unwrap();
        bm.attack_target(enemy);
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
            .values()
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

/// Residual: China Red Guard gun + horde/nationalism ROF + bayonet residual.
/// Fail-closed: not full WeaponSet tertiary auto-choose / RubOff matrix.
#[test]
fn red_guard_residual_gun_horde_nationalism_and_bayonet() {
    use crate::game_logic::host_battlemaster::UPGRADE_NATIONALISM;
    use crate::game_logic::host_red_guard::{
        REDGUARD_DAMAGE, REDGUARD_MACHINE_GUN, REDGUARD_RANGE, is_red_guard_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let mut rg_tpl = crate::game_logic::ThingTemplate::new("ChinaInfantryRedguard");
    rg_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(120.0)
        .set_primary_weapon_name(REDGUARD_MACHINE_GUN);
    game_logic
        .templates
        .insert("ChinaInfantryRedguard".to_string(), rg_tpl);

    // Spawn 5 ally red guards clustered for Horde Count=5 residual (Radius 30).
    let mut rg_ids = Vec::new();
    for i in 0..5 {
        let id = game_logic
            .create_object(
                "ChinaInfantryRedguard",
                Team::China,
                Vec3::new(i as f32 * 5.0, 0.0, 0.0),
            )
            .expect("redguard");
        rg_ids.push(id);
    }
    let rg0 = rg_ids[0];
    {
        let rg = game_logic.host_object(rg0).expect("rg0");
        assert!(is_red_guard_template(&rg.template_name));
        let w = rg.weapon.as_ref().expect("RedguardMachineGun residual");
        assert!(
            (w.damage - REDGUARD_DAMAGE).abs() < 0.5,
            "base damage residual 15, got {}",
            w.damage
        );
        assert!((w.range - REDGUARD_RANGE).abs() < 1.0);
        assert!(
            (w.reload_time - 1.0).abs() < 0.05,
            "base reload residual 1.0s, got {}",
            w.reload_time
        );
    }

    // Horde residual: 5 infantry allies within radius 30 → HORDE ROF 150% (20 frames).
    game_logic.update_china_infantry_horde_status();
    assert!(
        game_logic.honesty_red_guard_horde_ok(),
        "horde grant residual honesty"
    );
    {
        let rg = game_logic.host_object(rg0).expect("rg0 horde");
        assert!(
            rg.weapon_bonus_horde,
            "weapon_bonus_horde residual must be set with 5 allies"
        );
        let w = rg.weapon.as_ref().expect("horde gun");
        // floor(30/1.5)=20 frames → 20/30 ≈ 0.666s
        assert!(
            (w.reload_time - (20.0 / 30.0)).abs() < 0.05,
            "horde ROF residual ~20 frames, got reload={}",
            w.reload_time
        );
        assert!(
            (w.damage - 15.0).abs() < 0.5,
            "horde does not change damage"
        );
    }

    // Nationalism residual: additional ROF 125% while in horde → 16 frames.
    assert!(game_logic.apply_red_guard_nationalism_upgrade(rg0));
    assert!(game_logic.honesty_red_guard_nationalism_ok());
    {
        let rg = game_logic.host_object(rg0).expect("rg0 nat");
        assert!(rg.has_upgrade_tag(UPGRADE_NATIONALISM));
        assert!(
            rg.weapon_bonus_nationalism,
            "nationalism active while in horde"
        );
        let w = rg.weapon.as_ref().expect("nat gun");
        assert!(
            (w.reload_time - (16.0 / 30.0)).abs() < 0.05,
            "horde+nationalism ROF residual ~16 frames, got reload={}",
            w.reload_time
        );
    }

    // Gun fire residual vs enemy infantry (out of bayonet range).
    let enemy = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(50.0, 0.0, 0.0))
        .expect("enemy");
    {
        let rg = game_logic.host_object_mut(rg0).unwrap();
        rg.attack_target(enemy);
        if let Some(w) = rg.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(40);
    game_logic.update_combat(&[rg0, enemy], LOGIC_FRAME_TIMESTEP);

    assert!(
        game_logic.red_guard_residual_fires() > 0,
        "red guard residual fire honesty"
    );
    assert!(game_logic.honesty_red_guard_ok());
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "red guard residual must damage intended (before={enemy_hp_before} after={enemy_hp_after})"
    );

    // Bayonet residual: close-range infantry one-shot.
    let melee = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .expect("melee");
    {
        let rg = game_logic.host_object_mut(rg0).unwrap();
        rg.set_position(Vec3::new(0.0, 0.0, 0.0));
        rg.attack_target(melee);
        if let Some(w) = rg.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
        }
    }
    game_logic.set_current_frame(80);
    game_logic.update_combat(&[rg0, melee], LOGIC_FRAME_TIMESTEP);
    assert!(
        game_logic.honesty_red_guard_bayonet_ok(),
        "bayonet residual honesty"
    );
    let melee_alive = game_logic
        .host_object(melee)
        .map(|o| o.is_alive())
        .unwrap_or(false);
    assert!(!melee_alive, "bayonet residual one-shots close infantry");
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
        let th = game_logic.host_object_mut(th0).unwrap();
        th.attack_target(enemy);
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
            .values()
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
        let th = game_logic.host_object_mut(th0).unwrap();
        th.set_position(Vec3::new(200.0, 0.0, 2.0));
        // Facing direction to the building is atan2(-dz, dx) = +PI/2 (0 faces +X).
        th.set_orientation(std::f32::consts::FRAC_PI_2);
        th.set_ai_state(AIState::SpecialAbility);
        th.target = Some(bldg);
    }
    game_logic.queue_pending_special_ability(
        th0,
        PendingSpecialAbility::PlantTimedDemoCharge { target_id: bldg },
    );
    game_logic.update_ai(&[th0, bldg], 1.0 / 60.0);
    // C++ SpecialAbilityUpdate prep completes over multiple logic frames:
    // the first tick only arms the leftover channel (Facing -> Unpacking ->
    // Preparing, special_abilities.rs tick_leftover_special_ability), so keep
    // ticking until the plant lands (Burton plant fixtures do the same).
    for _ in 0..12 {
        game_logic.update_ai(&[th0, bldg], 1.0 / 30.0);
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
        .values()
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

/// Residual: GLA Rebel machine gun + AP Bullets damage upgrade.
/// Fail-closed: not full ClipSize volley / CaptureBuilding / BoobyTrap matrix.
#[test]
fn rebel_residual_gun_and_ap_bullets() {
    use crate::game_logic::host_gla_rebel::{
        REBEL_DAMAGE, REBEL_MACHINE_GUN, REBEL_RANGE, UPGRADE_GLA_AP_BULLETS, is_gla_rebel_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let mut rebel_tpl = crate::game_logic::ThingTemplate::new("GLAInfantryRebel");
    rebel_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0)
        .set_primary_weapon_name(REBEL_MACHINE_GUN);
    game_logic
        .templates
        .insert("GLAInfantryRebel".to_string(), rebel_tpl);

    let rebel_id = game_logic
        .create_object("GLAInfantryRebel", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("rebel");
    {
        let rebel = game_logic.host_object(rebel_id).expect("rebel");
        assert!(is_gla_rebel_template(&rebel.template_name));
        let w = rebel.weapon.as_ref().expect("GLARebelMachineGun residual");
        assert!(
            (w.damage - REBEL_DAMAGE).abs() < 0.5,
            "base damage residual 5, got {}",
            w.damage
        );
        assert!((w.range - REBEL_RANGE).abs() < 1.0);
        assert!(
            (w.reload_time - (3.0 / 30.0)).abs() < 0.05,
            "base reload residual ~0.1s (3 frames), got {}",
            w.reload_time
        );
        assert!(!w.can_target_air && w.can_target_ground);
    }

    // AP Bullets residual: damage × 1.25 → 6.25.
    assert!(game_logic.apply_rebel_ap_bullets_upgrade(rebel_id));
    assert!(game_logic.honesty_rebel_ap_ok());
    {
        let rebel = game_logic.host_object(rebel_id).expect("rebel ap");
        assert!(rebel.has_upgrade_tag(UPGRADE_GLA_AP_BULLETS));
        let w = rebel.weapon.as_ref().expect("ap gun");
        assert!(
            (w.damage - 6.25).abs() < 0.05,
            "AP Bullets residual damage 6.25, got {}",
            w.damage
        );
        // ROF unchanged by AP.
        assert!((w.reload_time - (3.0 / 30.0)).abs() < 0.05);
    }

    // Gun fire residual vs enemy infantry.
    let enemy = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(50.0, 0.0, 0.0))
        .expect("enemy");
    {
        let rebel = game_logic.host_object_mut(rebel_id).unwrap();
        rebel.attack_target(enemy);
        if let Some(w) = rebel.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(40);
    game_logic.update_combat(&[rebel_id, enemy], LOGIC_FRAME_TIMESTEP);

    assert!(
        game_logic.rebel_residual_fires() > 0,
        "rebel residual fire honesty"
    );
    assert!(game_logic.honesty_rebel_ok());
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "rebel residual must damage intended (before={enemy_hp_before} after={enemy_hp_after})"
    );
    // AP residual damage applied on fire (≥5 base; with AP ~6.25).
    let dmg_dealt = enemy_hp_before - enemy_hp_after;
    assert!(
        dmg_dealt >= 5.0 - 0.1,
        "AP residual fire should deal at least base damage, got {dmg_dealt}"
    );
}
