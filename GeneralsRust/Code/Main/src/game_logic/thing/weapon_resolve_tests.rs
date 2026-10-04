use super::*;

#[test]
fn explicit_primary_weapon_beats_store_and_default() {
    let mut t = ThingTemplate::new("Armed");
    t.add_kind_of(KindOf::Infantry);
    t.set_primary_weapon(Weapon {
        damage: 40.0,
        range: 80.0,
        reload_time: 0.5,
        ..Weapon::default()
    });
    t.set_primary_weapon_name("DoesNotExistInStoreHopefully");
    let w = t.resolve_primary_weapon().expect("weapon");
    assert!((w.damage - 40.0).abs() < 0.01);
    assert!((w.range - 80.0).abs() < 0.01);
}

#[test]
fn infantry_without_weapon_gets_kind_fallback() {
    let mut t = ThingTemplate::new("BareInfantry");
    t.add_kind_of(KindOf::Infantry);
    let w = t.resolve_primary_weapon().expect("fallback");
    assert!((w.damage - Weapon::default().damage).abs() < 0.01);
}

#[test]
fn structure_without_weapon_stays_unarmed() {
    let mut t = ThingTemplate::new("BareStructure");
    t.add_kind_of(KindOf::Structure);
    assert!(t.resolve_primary_weapon().is_none());
}

#[test]
fn primary_weapon_name_resolves_non_default_store_stats() {
    // Prove store bind path for USA_Ranger / GoldenRanger weapon name.
    let mut t = ThingTemplate::new("USA_Ranger");
    t.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .set_primary_weapon_name(super::super::weapon_bootstrap::RANGER_PRIMARY_WEAPON);
    let w = t.resolve_primary_weapon().expect("store-bound weapon");
    assert!(
        (w.damage - Weapon::default().damage).abs() > 0.01,
        "store path must not yield host default damage; got {}",
        w.damage
    );
    assert!((w.damage - 5.0).abs() < 0.01);
    assert!((w.range - 100.0).abs() < 0.01);
}

#[test]
fn secondary_weapon_name_resolves_non_default_store_stats() {
    // Prove SECONDARY store bind path (Ranger flashbang residual).
    let mut t = ThingTemplate::new("USA_Ranger");
    t.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .set_secondary_weapon_name(super::super::weapon_bootstrap::RANGER_SECONDARY_WEAPON);
    let w = t.resolve_secondary_weapon().expect("store-bound secondary");
    assert!(
        (w.damage - Weapon::default().damage).abs() > 0.01,
        "secondary store path must not yield host default damage; got {}",
        w.damage
    );
    // Retail RangerFlashBangGrenadeWeapon PrimaryDamage 35, AttackRange 175.
    // Store the authored 175; runtime getAttackRange derives 172.5.
    assert!((w.damage - 35.0).abs() < 0.01);
    assert!((w.range - 175.0).abs() < 0.01);
}

#[test]
fn secondary_without_name_stays_none_even_for_infantry() {
    // Fail-closed: no kind-based default for secondary slots.
    let mut t = ThingTemplate::new("BareInfantry");
    t.add_kind_of(KindOf::Infantry);
    assert!(t.resolve_secondary_weapon().is_none());
}

#[test]
fn unit_name_residual_map_binds_without_explicit_weapon_name() {
    // units.rs / setup_templates often omit primary_weapon_name; residual map
    // must still prefer retail store stats over kind-based Weapon::default.
    let mut technical = ThingTemplate::new("GLA_Technical");
    technical
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable);
    let tw = technical
        .resolve_primary_weapon()
        .expect("technical residual weapon");
    assert!(
        (tw.damage - Weapon::default().damage).abs() > 0.01,
        "GLA_Technical must not fall through to Weapon::default (got dmg={})",
        tw.damage
    );
    // Retail TechnicalMachineGunWeapon PrimaryDamage 10.
    assert!((tw.damage - 10.0).abs() < 0.01);
    assert!((tw.range - 150.0).abs() < 0.01);

    let mut battle = ThingTemplate::new("China_BattleTank");
    battle
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable);
    let bw = battle
        .resolve_primary_weapon()
        .expect("battlemaster residual weapon");
    assert!(
        (bw.damage - Weapon::default().damage).abs() > 0.01,
        "China_BattleTank must not fall through to Weapon::default (got dmg={})",
        bw.damage
    );
    // Retail BattleMasterTankGun PrimaryDamage 60.
    assert!((bw.damage - 60.0).abs() < 0.01);
    assert!((bw.range - 150.0).abs() < 0.01);
}

#[test]
fn secondary_unit_name_residual_map_binds_ranger_flashbang() {
    let mut t = ThingTemplate::new("AmericaInfantryColonelBurton");
    t.set_secondary_weapon(Weapon {
        damage: 99.0,
        range: 50.0,
        reload_time: 1.0,
        ..Weapon::default()
    });
    t.set_secondary_weapon_name("DoesNotExistInStoreHopefully");
    let w = t.resolve_secondary_weapon().expect("weapon");
    assert!((w.damage - 99.0).abs() < 0.01);
    assert!((w.range - 50.0).abs() < 0.01);
}

#[test]
fn preferred_against_ini_parses_slot_and_kinds() {
    // C++ WeaponSet.cpp:119-122 parsePreferredAgainst: slot then KindOf list.
    let (slot, kinds) = parse_preferred_against_value("PRIMARY INFANTRY").unwrap();
    assert_eq!(slot, 0);
    assert_eq!(kinds, vec![KindOf::Infantry]);
    let (slot, kinds) =
        parse_preferred_against_value("SECONDARY AIRCRAFT BALLISTIC_MISSILE").unwrap();
    assert_eq!(slot, 1);
    assert_eq!(kinds, vec![KindOf::Aircraft, KindOf::BallisticMissile]);
}

#[test]
fn apply_weapon_set_definition_binds_preferred_and_share_reload() {
    let mut set = crate::assets::WeaponSetDefinition::default();
    set.attributes.insert(
        "PreferredAgainst".to_string(),
        "PRIMARY INFANTRY".to_string(),
    );
    set.attributes
        .insert("ShareWeaponReloadTime".to_string(), "Yes".to_string());
    set.attributes.insert(
        "AutoChooseSources".to_string(),
        "SECONDARY NONE".to_string(),
    );
    set.attributes
        .insert("WeaponLockSharedAcrossSets".to_string(), "No".to_string());
    let mut t = ThingTemplate::new("AmericaVehicleComanche");
    t.apply_weapon_set_definition(&set);
    assert_eq!(t.preferred_against[0], vec![KindOf::Infantry]);
    assert!(t.share_weapon_reload_time);
    assert!(t.slot_preferred_against(0, |k| k == KindOf::Infantry));
    assert!(!t.slot_preferred_against(0, |k| k == KindOf::Vehicle));
    assert_eq!(t.auto_choose_masks[1], 0);
    assert!(t.slot_allows_auto_choose(0));
    assert!(!t.slot_allows_auto_choose(1));
    assert!(!t.weapon_lock_shared_across_sets);
}

#[test]
fn weapon_tracker_from_store_maps_int_max_to_unbound() {
    // C++ WeaponTemplate defaults ContinuousFireOne/Two to INT_MAX (off).
    let bind = ThingTemplate::weapon_tracker_from_store(
        super::super::weapon_bootstrap::RANGER_PRIMARY_WEAPON,
    );
    assert_eq!(bind.continuous_fire_one_shots, u32::MAX);
    assert_eq!(bind.continuous_fire_two_shots, u32::MAX);
}

#[test]
fn pack_unpack_variation_matches_cpp_inclusive_range() {
    // SpecialAbilityUpdate.cpp:721/774 GameLogicRandomValueReal(1-f, 1+f).
    assert_eq!(
        apply_pack_unpack_variation_ms(5500, pack_unpack_variation_multiplier(0.2, 0.0)),
        4400
    );
    assert_eq!(
        apply_pack_unpack_variation_ms(5500, pack_unpack_variation_multiplier(0.2, 1.0)),
        6600
    );
    assert_eq!(
        apply_pack_unpack_variation_ms(5500, pack_unpack_variation_multiplier(0.0, 0.37)),
        5500
    );
    assert_eq!(vary_pack_unpack_duration_ms(0, 0.5), 0);
    assert_eq!(vary_pack_unpack_duration_ms(5500, 0.0), 5500);
}

#[test]
fn weapon_from_store_uses_leftover_delay_not_max_flatten() {
    // Old flatten treated max as clip and used max(min,max) when clip_size==0.
    // Leftover get_delay_between_shots yardstick is Min (Weapon.cpp:475-490).
    const NAME: &str = "__RustLiveDelayBetweenShotsRange";
    let _ = super::super::weapon_bootstrap::ensure_host_weapon_store();
    let _ = gamelogic::weapon::with_weapon_store_mut(|store| {
        let mut template = gamelogic::weapon::WeaponTemplate::new(NAME.to_string());
        template.primary_damage = 10.0;
        template.attack_range = 100.0;
        template.min_delay_between_shots = 6;
        template.max_delay_between_shots = 30;
        template.clip_size = 0;
        store.add_weapon_template(template);
    });
    let weapon = ThingTemplate::weapon_from_store(NAME).expect("store weapon");
    let leftover = {
        let mut yardstick = gamelogic::weapon::WeaponTemplate::new(NAME.to_string());
        yardstick.min_delay_between_shots = 6;
        yardstick.max_delay_between_shots = 6;
        yardstick.get_delay_between_shots(&gamelogic::weapon::WeaponBonus::new())
    };
    assert_eq!(leftover, 6);
    assert!(
        (weapon.reload_time - leftover as f32 / 30.0).abs() < 1e-6,
        "reload_time={} leftover_min={} flattened_max={}",
        weapon.reload_time,
        leftover as f32 / 30.0,
        30.0 / 30.0
    );
    let rof = 2.0;
    let with_rof =
        super::super::weapon_bootstrap::host_delay_between_shots_secs_nominal_with_rof(NAME, rof)
            .expect("leftover ROF yardstick");
    // leftover REAL_TO_INT_FLOOR(6 / 2) = 3 frames → 0.1s, not (6/30)/2.
    assert!((with_rof - 3.0 / 30.0).abs() < 1e-6, "with_rof={with_rof}");
}

#[test]
fn weapon_from_store_uses_leftover_rationalize_attack_range() {
    // Stored host fields keep authored values. The unchanged native
    // getters and the actual Object runtime derive one quarter-cell less.
    const NAME: &str = "__RustLiveRationalizeAttackRange";
    let _ = super::super::weapon_bootstrap::ensure_host_weapon_store();
    let _ = gamelogic::weapon::with_weapon_store_mut(|store| {
        let mut template = gamelogic::weapon::WeaponTemplate::new(NAME.to_string());
        template.primary_damage = 10.0;
        template.attack_range = 100.0;
        template.minimum_attack_range = 10.0;
        store.add_weapon_template(template);
    });
    let weapon = ThingTemplate::weapon_from_store(NAME).expect("store weapon");
    let leftover = {
        let mut yardstick = gamelogic::weapon::WeaponTemplate::new(NAME.to_string());
        yardstick.attack_range = 100.0;
        yardstick.minimum_attack_range = 10.0;
        (
            yardstick.get_attack_range(&gamelogic::weapon::WeaponBonus::new()),
            yardstick.get_minimum_attack_range(),
            yardstick.is_contact_weapon(),
        )
    };
    assert!(
        (leftover.0 - 97.5).abs() < 1e-6,
        "leftover max={}",
        leftover.0
    );
    assert!(
        (leftover.1 - 7.5).abs() < 1e-6,
        "leftover min={}",
        leftover.1
    );
    assert!(!leftover.2);
    assert_eq!((weapon.range, weapon.min_range), (100.0, 10.0));
    let mut template = ThingTemplate::new("RuntimeRationalizeAttackRange");
    template.set_primary_weapon_name(NAME);
    let mut logic = crate::game_logic::GameLogic::new();
    logic.templates.insert(template.name.clone(), template);
    let id = logic
        .create_object(
            "RuntimeRationalizeAttackRange",
            crate::game_logic::Team::USA,
            Vec3::ZERO,
        )
        .expect("actual named weapon admission");
    let object = &logic.objects[&id];
    assert!(!object.is_within_attack_range_at_distance(0, leftover.1 - 0.01));
    assert!(object.is_within_attack_range_at_distance(0, leftover.1));
    assert!(object.is_within_attack_range_at_distance(0, leftover.0));
    assert!(!object.is_within_attack_range_at_distance(0, leftover.0 + 0.01));
}

#[test]
fn leftover_is_contact_weapon_authored_range_under_12_5() {
    let mut contact = gamelogic::weapon::WeaponTemplate::new("c".into());
    contact.attack_range = 10.0;
    assert!(contact.is_contact_weapon());
    let mut edge = gamelogic::weapon::WeaponTemplate::new("e".into());
    edge.attack_range = 12.5;
    assert!(!edge.is_contact_weapon());
    assert!(super::super::weapon_bootstrap::is_contact_weapon_range(
        10.0
    ));
    assert!(!super::super::weapon_bootstrap::is_contact_weapon_range(
        12.5
    ));

    // Authored 10: leftover/C++ is contact (7.5 < 10); #else FUDGE was not.
    const CONTACT: &str = "__RustLiveContactAuthored10";
    const EDGE: &str = "__RustLiveContactAuthored12_5";
    let _ = super::super::weapon_bootstrap::ensure_host_weapon_store();
    let _ = gamelogic::weapon::with_weapon_store_mut(|store| {
        let mut t = gamelogic::weapon::WeaponTemplate::new(CONTACT.to_string());
        t.primary_damage = 10.0;
        t.attack_range = 10.0;
        store.add_weapon_template(t);
        let mut t = gamelogic::weapon::WeaponTemplate::new(EDGE.to_string());
        t.primary_damage = 10.0;
        t.attack_range = 12.5;
        store.add_weapon_template(t);
    });
    assert!(super::super::weapon_bootstrap::host_is_contact_weapon_name(
        CONTACT
    ));
    assert!(!super::super::weapon_bootstrap::host_is_contact_weapon_name(EDGE));
    let w = ThingTemplate::weapon_from_store(CONTACT).expect("contact store");
    assert_eq!(w.range, 10.0);
    assert!(super::super::weapon_bootstrap::is_contact_weapon_range(
        w.range
    ));
    let edge_w = ThingTemplate::weapon_from_store(EDGE).expect("edge store");
    assert_eq!(edge_w.range, 12.5);
    assert!(!super::super::weapon_bootstrap::is_contact_weapon_range(
        edge_w.range
    ));
}
