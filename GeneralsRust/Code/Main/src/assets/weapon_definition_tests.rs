//! The actual archive parser kernel owns complete effective native rules.
//! No registry registration or ambient catalogue is used by these controls.
use super::*;
#[test]
fn owned_archive_definitions_keep_partial_ranger_and_admit_complete_missile_rules() {
    let mut definitions = HashMap::new();
    assert_eq!(
        parse_weapon_definitions_ini_into(
            &mut definitions,
            "Weapon RangerAdvancedCombatRifle\nPrimaryDamage = 5\nEnd\n",
            WeaponDefinitionLoadType::Normal
        ),
        1
    );
    assert_eq!(
        parse_weapon_definitions_ini_into(
            &mut definitions,
            "Weapon HellfireMissileWeapon\nPrimaryDamage = 40\nPrimaryDamageRadius = 5\nAttackRange = 200\nWeaponSpeed = 600\nMinWeaponSpeed = 300\nProjectileObject = MissileDefenderMissile\nEnd\n",
            WeaponDefinitionLoadType::Normal
        ),
        1
    );
    let missile = &definitions["hellfiremissileweapon"];
    assert_eq!(missile.primary_damage, 40.0);
    assert_eq!(missile.primary_damage_radius, 5.0);
    assert_eq!(
        missile.weapon_speed,
        600.0_f32 * (1.0_f32 / 30.0_f32),
        "CPP parseVelocityReal multiplies by SECONDS_PER_LOGICFRAME_REAL"
    );
    assert_eq!(missile.min_weapon_speed, 300.0_f32 * (1.0_f32 / 30.0_f32));
    assert_eq!(missile.projectile_name, "MissileDefenderMissile");
    assert_eq!(definitions["rangeradvancedcombatrifle"].primary_damage, 5.0);
    assert_eq!(owned_weapon_snapshot(definitions).len(), 2);
}
#[test]
fn owned_override_preserves_base_and_repeated_rank_fields_in_source_order() {
    let mut definitions = HashMap::new();
    parse_weapon_definitions_ini_into(
        &mut definitions,
        "Weapon SameRule\nPrimaryDamage = 10\nAttackRange = 150\nWeaponSpeed = 600\nFireFX = BaseFX\nVeterancyFireFX = VETERAN VeteranFX\nVeterancyFireFX = HEROIC HeroFX\nEnd\n",
        WeaponDefinitionLoadType::Normal,
    );
    let original = definitions["samerule"].clone();
    parse_weapon_definitions_ini_into(
        &mut definitions,
        "Weapon samerule\nPrimaryDamage = 25\nVeterancyFireFX = ELITE EliteFX\nVeterancyFireFX = HEROIC None\nEnd\n",
        WeaponDefinitionLoadType::CreateOverrides,
    );
    let selected = &definitions["samerule"];
    assert_eq!(selected.name, "SameRule");
    assert_eq!(selected.primary_damage, 25.0);
    assert_eq!(selected.attack_range, 150.0);
    assert_eq!(selected.weapon_speed, 600.0_f32 * (1.0_f32 / 30.0_f32));
    use gamelogic::common::VeterancyLevel::*;
    assert_eq!(selected.get_fire_fx(Regular).unwrap().name(), "BaseFX");
    assert_eq!(selected.get_fire_fx(Veteran).unwrap().name(), "VeteranFX");
    assert_eq!(selected.get_fire_fx(Elite).unwrap().name(), "EliteFX");
    assert!(selected.get_fire_fx(Heroic).is_none());
    assert_eq!(
        original.primary_damage, 10.0,
        "base value remains immutable"
    );
    assert_eq!(original.get_fire_fx(Heroic).unwrap().name(), "HeroFX");
    assert_eq!(definitions.len(), 1);
}

#[test]
fn owned_archive_shot_delay_prefixes_set_host_weapon_cadence() {
    let mut definitions = HashMap::new();
    let count = parse_weapon_definitions_ini_into(
        &mut definitions,
        r#"Weapon BattleShipGunReal
PrimaryDamage = 1000
DelayBetweenShots = 250.0
End
Weapon __ShotDelayMinMax
DelayBetweenShots = Min:34.99 Max:250.9
End
"#,
        WeaponDefinitionLoadType::Normal,
    );

    assert_eq!(count, 2);
    let battleship = &definitions["battleshipgunreal"];
    assert_eq!(battleship.min_delay_between_shots, 8);
    assert_eq!(battleship.max_delay_between_shots, 8);

    let range = &definitions["__shotdelayminmax"];
    assert_eq!(range.min_delay_between_shots, 2);
    assert_eq!(range.max_delay_between_shots, 8);
}
#[test]
fn independent_owned_loads_keep_defaults_and_lookup_sorting_out_of_override_order() {
    let mut first = HashMap::new();
    let mut peer = HashMap::new();
    parse_weapon_definitions_ini_into(
        &mut first,
        "Weapon ZLast\nPrimaryDamage = 1\nEnd\nWeapon SameRule\nPrimaryDamage = 3\nEnd\nWeapon SameRule\nPrimaryDamage = 7\nEnd\n",
        WeaponDefinitionLoadType::CreateOverrides,
    );
    parse_weapon_definitions_ini_into(
        &mut peer,
        "Weapon SameRule\nPrimaryDamage = 99\nWeaponSpeed = 300\nEnd\n",
        WeaponDefinitionLoadType::Normal,
    );
    assert_eq!(first["samerule"].primary_damage, 7.0);
    assert_eq!(peer["samerule"].primary_damage, 99.0);
    assert_eq!(
        peer["samerule"].weapon_speed,
        300.0_f32 * (1.0_f32 / 30.0_f32)
    );
    let defaults = gamelogic::weapon::WeaponTemplate::new("BaselineDefaults".into());
    assert_eq!(first["samerule"].weapon_speed, defaults.weapon_speed);
    assert_eq!(first["samerule"].damage_type, defaults.damage_type);
    let snapshot = owned_weapon_snapshot(first);
    assert_eq!(
        snapshot
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        vec!["SameRule", "ZLast"]
    );
    assert_eq!(snapshot[0].1.primary_damage, 7.0);
}

#[test]
fn normal_duplicate_keeps_original_while_explicit_map_override_replaces_it() {
    let mut definitions = HashMap::new();
    assert_eq!(
        parse_weapon_definitions_ini_into(
            &mut definitions,
            "Weapon DuplicateRule\nPrimaryDamage = 10\nWeaponSpeed = 600\nEnd\n",
            WeaponDefinitionLoadType::Normal
        ),
        1
    );
    assert_eq!(
        parse_weapon_definitions_ini_into(
            &mut definitions,
            "Weapon duplicaterule\nPrimaryDamage = 99\nEnd\n",
            WeaponDefinitionLoadType::Normal
        ),
        0
    );
    assert_eq!(definitions["duplicaterule"].primary_damage, 10.0);
    assert_eq!(definitions.len(), 1);
    assert_eq!(
        parse_weapon_definitions_ini_into(
            &mut definitions,
            "Weapon duplicaterule\nPrimaryDamage = 99\nEnd\n",
            WeaponDefinitionLoadType::CreateOverrides
        ),
        1
    );
    assert_eq!(definitions["duplicaterule"].primary_damage, 99.0);
    assert_eq!(
        definitions["duplicaterule"].weapon_speed,
        600.0_f32 * (1.0_f32 / 30.0_f32)
    );
    assert_eq!(definitions.len(), 1);
}
