use generals_main::game_logic::{
    AbleToAttackType, CanAttackResult, GameLogic, ObjectId, Player, Team,
};
use glam::Vec3;

fn world(range_percent: u32, slot: &str) -> (GameLogic, ObjectId, ObjectId) {
    let mut logic = GameLogic::new();
    logic
        .admit_combat_bonus_ini_sources([format!(
            "GameData\n WeaponBonus = SOLO_HUMAN_EASY RANGE {range_percent}%\nEnd\n"
        )])
        .expect("receiving GameData accepted");
    let mut owner = Player::new(42, Team::USA, "Owner", true);
    owner.alliance_team = 1;
    let mut target_owner = Player::new(43, Team::GLA, "Target", false);
    target_owner.alliance_team = 2;
    logic.add_player(owner);
    logic.add_player(target_owner);

    logic
        .admit_object_ini_sources([(
            "RangeBonus.ini".into(),
            format!(
                "Object RangeBonusAttacker
 KindOf = VEHICLE CAN_ATTACK
 Body = ActiveBody Body
  MaxHealth = 100
  InitialHealth = 100
 End
 WeaponSet
  Conditions = None
  Weapon = {slot} RangeBonusGun
 End
End
\
             Object RangeBonusTarget
 KindOf = VEHICLE SELECTABLE
 Body = ActiveBody Body
  MaxHealth = 100
  InitialHealth = 100
 End
End
"
            ),
        )])
        .unwrap();
    let mut weapon = generals_sim::weapon::WeaponDefinition::new("RangeBonusGun".into());
    weapon.primary_damage = 10.0;
    weapon.attack_range = 100.0;
    weapon.clip_size = 4;
    weapon.anti_mask =
        generals_sim::weapon::WeaponAntiMask::new(generals_sim::weapon::WeaponAntiMask::GROUND);
    logic
        .admit_receiving_weapon_snapshot(vec![(weapon.name.clone(), weapon)])
        .unwrap();

    let attacker = logic
        .create_object_for_player("RangeBonusAttacker", 42, Vec3::ZERO)
        .expect("attacker admitted");
    let target = logic
        .create_object_for_player("RangeBonusTarget", 43, Vec3::new(120.0, 0.0, 0.0))
        .expect("target admitted");
    logic
        .host_object_mut(attacker)
        .expect("attacker object")
        .weapon_bonus_solo =
        generals_main::game_logic::host_enum_table_residual::WEAPON_BONUS_SOLO_HUMAN_EASY_ORDINAL
            as u8;
    (logic, attacker, target)
}

#[test]
fn attack_range_uses_receiving_solo_bonus_for_same_ids() {
    let (boosted, boosted_attacker, boosted_target) = world(200, "PRIMARY");
    let (neutral, neutral_attacker, neutral_target) = world(100, "PRIMARY");
    assert_eq!(boosted_attacker, neutral_attacker);
    assert_eq!(boosted_target, neutral_target);

    // This public GameLogic query is used by attack commands and the host AI
    // state machine. Each receiver must use its own admitted WeaponBonusSet.
    assert_eq!(
        boosted.get_able_to_attack_specific_object(
            boosted_attacker,
            boosted_target,
            AbleToAttackType::NewTarget,
            false,
        ),
        CanAttackResult::Possible,
    );
    assert_eq!(
        neutral.get_able_to_attack_specific_object(
            neutral_attacker,
            neutral_target,
            AbleToAttackType::NewTarget,
            false,
        ),
        CanAttackResult::PossibleAfterMoving,
    );
    assert!(!boosted.out_of_weapon_range_object(boosted_attacker, boosted_target));
    assert!(neutral.out_of_weapon_range_object(neutral_attacker, neutral_target));
}

#[test]
fn ai_decision_uses_receiving_solo_bonus_for_same_ids() {
    use generals_main::ai_decisions::{AIDecisionSystem, AttackDecision};

    let (boosted, boosted_attacker, boosted_target) = world(200, "PRIMARY");
    let (reduced, reduced_attacker, reduced_target) = world(50, "PRIMARY");
    assert_eq!(
        (boosted_attacker, boosted_target),
        (reduced_attacker, reduced_target)
    );
    assert_eq!(
        AIDecisionSystem::should_attack(&boosted, boosted_attacker, boosted_target),
        AttackDecision::Attack,
    );
    assert_eq!(
        AIDecisionSystem::should_attack(&reduced, reduced_attacker, reduced_target),
        AttackDecision::Hold,
    );
}

#[test]
fn explicit_secondary_target_selection_uses_receiving_bonus() {
    let (mut boosted, boosted_attacker, boosted_target) = world(200, "SECONDARY");
    let (mut reduced, reduced_attacker, reduced_target) = world(50, "SECONDARY");
    for (world, attacker, target) in [
        (&mut boosted, boosted_attacker, boosted_target),
        (&mut reduced, reduced_attacker, reduced_target),
    ] {
        // C++ WeaponSet.cpp:793–943 can select a compatible weapon for
        // approach. The receiving range bonus is checked at fire admission.
        assert!(world.choose_best_weapon_for_target(attacker, Some(target), 10.0));
        assert_eq!(
            world.host_object(attacker).unwrap().selected_weapon_slot(),
            Some(generals_sim::weapon::WeaponSlotType::Secondary as u8),
        );
    }
    assert!(boosted.attack_can_fire_at(boosted_attacker, boosted_target, 10.0, false));
    assert!(!reduced.attack_can_fire_at(reduced_attacker, reduced_target, 10.0, false));
}

#[test]
fn attack_fire_gate_uses_receiving_bonus() {
    let (boosted, boosted_attacker, boosted_target) = world(200, "PRIMARY");
    let (reduced, reduced_attacker, reduced_target) = world(50, "PRIMARY");
    assert!(boosted.attack_can_fire_at(boosted_attacker, boosted_target, 10.0, false));
    assert!(!reduced.attack_can_fire_at(reduced_attacker, reduced_target, 10.0, false));
}

fn authored_pitch_world(
    primary_damage: f32,
    primary_pitch: f32,
    secondary_pitch: Option<f32>,
    legacy_pitch: f32,
) -> (GameLogic, ObjectId, ObjectId) {
    use generals_sim::weapon::WeaponDefinition as WeaponTemplate;
    use generals_sim::weapon::WeaponAntiMask;

    fn rule(name: &str, damage: f32, max_pitch: f32) -> WeaponTemplate {
        let mut rule = WeaponTemplate::new(name.into());
        rule.primary_damage = damage;
        rule.attack_range = 150.0;
        rule.min_target_pitch = -max_pitch;
        rule.max_target_pitch = max_pitch;
        rule.anti_mask = WeaponAntiMask::new(WeaponAntiMask::GROUND);
        rule
    }

    let mut world = GameLogic::new();
    let primary = rule("SharedPitchPrimary", primary_damage, primary_pitch);
    let secondary = secondary_pitch.map(|pitch| rule("SharedPitchSecondary", 20.0, pitch));
    let secondary_slot = if secondary.is_some() {
        "  Weapon = SECONDARY SharedPitchSecondary\n"
    } else {
        ""
    };
    world
        .admit_object_ini_sources([(
            "PitchOwners.ini".into(),
            format!(
                "Object PitchAttacker\n KindOf = VEHICLE CAN_ATTACK\n Body = ActiveBody Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n WeaponSet\n  Conditions = None\n  Weapon = PRIMARY SharedPitchPrimary\n{secondary_slot} End\nEnd\n\
                 Object PitchVictim\n KindOf = INFANTRY\n Body = ActiveBody Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\nEnd\n"
            ),
        )])
        .unwrap();
    world
        .admit_receiving_weapon_snapshot(
            std::iter::once(primary)
                .chain(secondary)
                .map(|rule| (rule.name.clone(), rule))
                .collect(),
        )
        .unwrap();
    world.add_player(Player::new(42, Team::USA, "Owner", true));
    world.add_player(Player::new(43, Team::GLA, "Target", false));
    let source = world
        .create_object_for_player("PitchAttacker", 42, Vec3::ZERO)
        .unwrap();
    let target = world
        .create_object_for_player("PitchVictim", 43, Vec3::new(24.0, 50.0, 0.0))
        .unwrap();
    assert!(world.set_registered_object_position(target, Vec3::new(24.0, 50.0, 0.0)));
    world.with_own_services(|| {
        gamelogic::initialize_weapon_store().unwrap();
        gamelogic::weapon::with_weapon_store_mut(|store| {
            // The live objects already hold their own rows. Replace only the
            // unused compatibility catalog with a cold, conflicting fixture.
            *store = gamelogic::weapon::WeaponStore::new();
            store.add_weapon_template(rule("SharedPitchPrimary", 30.0, legacy_pitch));
            assert!(!store.host_bootstrap_is_complete());
        })
        .unwrap();
    });
    (world, source, target)
}

#[test]
fn authored_attack_eligibility_uses_receiving_pitch_without_bootstrapping_the_legacy_catalog() {
    let level = 15_f32.to_radians();
    let steep = 85_f32.to_radians();
    let (low, low_source, low_target) = authored_pitch_world(30.0, level, None, steep);
    let (high, high_source, high_target) = authored_pitch_world(30.0, steep, None, level);
    assert_eq!((low_source, low_target), (high_source, high_target));

    high.with_own_services(|| {
        let selected = gamelogic::system::engine_stores::active_services();
        for (world, expected) in [
            (&low, CanAttackResult::InvalidShot),
            (&high, CanAttackResult::Possible),
        ] {
            assert_eq!(
                world.get_able_to_use_weapon_against_target(
                    high_source,
                    Some(high_target),
                    None,
                    AbleToAttackType::NewTarget,
                ),
                expected,
            );
            assert!(std::sync::Arc::ptr_eq(
                &selected,
                &gamelogic::system::engine_stores::active_services(),
            ));
        }
    });
    for world in [&low, &high] {
        world.with_own_services(|| {
            assert!(
                !gamelogic::weapon::with_weapon_store(|store| store.host_bootstrap_is_complete())
                    .unwrap()
            );
        });
    }
}

#[test]
fn authored_attack_pitch_checks_every_slot_but_damage_respects_the_lock() {
    use generals_main::game_logic::WeaponLockType;

    let level = 15_f32.to_radians();
    let steep = 85_f32.to_radians();
    for (damage, expected) in [
        (30.0, CanAttackResult::Possible),
        (0.0, CanAttackResult::InvalidShot),
    ] {
        let (mut world, source, target) = authored_pitch_world(damage, level, Some(steep), level);
        assert!(
            world
                .host_object_mut(source)
                .unwrap()
                .set_weapon_lock(0, WeaponLockType::LockedPermanently)
        );
        assert_eq!(
            world.get_able_to_use_weapon_against_target(
                source,
                Some(target),
                None,
                AbleToAttackType::NewTarget,
            ),
            expected,
        );
    }
}
