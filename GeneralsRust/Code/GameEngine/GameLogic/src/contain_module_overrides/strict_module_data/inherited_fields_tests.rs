use super::*;
use crate::contain_module_overrides::ContainModuleDataKind;

const CASES: &[(&str, &str, &str)] = &[
    (
        "TunnelContain",
        "TimeForFullHeal = 1000ms",
        "TimeForFullHeal = invalid",
    ),
    (
        "HealContain",
        "TimeForFullHeal = 1500ms",
        "TimeForFullHeal = invalid",
    ),
    ("CaveContain", "CaveIndex = 3", "CaveIndex = invalid"),
    ("MobNexusContain", "Slots = 9", "Slots = invalid"),
    (
        "ParachuteContain",
        "LowAltitudeDamping = 0.4",
        "LowAltitudeDamping = invalid",
    ),
];

#[test]
fn actual_inherited_contain_data_reads_one_block_and_retains_following_header() {
    // C++ base table then derived table in one MultiIniFieldParse:
    // TunnelContain.h41, CaveContain.h31, HealContain.cpp,
    // MobNexusContain.cpp59, ParachuteContain.cpp50.
    let mut factory = catalog();
    for &(name, field, _) in CASES {
        let body =
            format!("ContainMax = 7\n{field}\nAllowAlliesInside = Yes\nEnd\nFollowingBlock\n");
        let data = INI::new()
            .with_inline_source(&body, |ini| {
                let data = factory
                    .construct(Some(ini), name, ModuleType::Behavior)
                    .unwrap();
                ini.read_line()?;
                assert_eq!(
                    ini.get_line_tokens(),
                    ["FollowingBlock"],
                    "{name} consumed its sibling"
                );
                Ok(data)
            })
            .unwrap();
        let base = match ContainModuleDataKind::from_module_data(data.as_ref()).expect(name) {
            ContainModuleDataKind::Tunnel(data) => {
                assert_eq!(data.frames_for_full_heal, 30.0);
                &data.base
            }
            ContainModuleDataKind::Heal(data) => {
                assert_eq!(data.frames_for_full_heal, 45);
                &data.base
            }
            ContainModuleDataKind::Cave(data) => {
                assert_eq!(data.cave_index_data, 3);
                &data.base
            }
            ContainModuleDataKind::MobNexus(data) => {
                assert_eq!(data.slot_capacity, 9);
                &data.base
            }
            ContainModuleDataKind::Parachute(data) => {
                assert_eq!(data.low_altitude_damping, 0.4);
                &data.base
            }
            _ => panic!("wrong actual contain data for {name}"),
        };
        assert_eq!(
            base.contain_max, 7,
            "{name} inherited field before derived field"
        );
        assert!(
            base.allow_allies_inside,
            "{name} inherited field after derived field"
        );
    }
}

#[test]
fn inherited_contain_parsers_reject_unknown_invalid_and_unterminated_fields() {
    let mut factory = catalog();
    for &(name, field, invalid) in CASES {
        for body in [
            format!("ContainMax = 7\n{field}\nUnknownContainField = 1\nEnd\n"),
            format!("ContainMax = 7\n{invalid}\nEnd\n"),
            format!("ContainMax = 7\n{field}\n"),
        ] {
            assert!(
                parse(&mut factory, ModuleType::Behavior, name, &body).is_err(),
                "{name}: {body}"
            );
        }
        assert!(
            parse(&mut factory, ModuleType::Behavior, name, "End").is_ok(),
            "{name} final unterminated line with End is valid"
        );
    }
}

#[test]
fn grant_stealth_keeps_authored_fields_and_rejects_unknown_data() {
    // GrantStealthBehavior.h44 calls UpdateModuleData's empty ancestor table.
    let mut factory = catalog();
    let data = parse(
        &mut factory,
        ModuleType::Behavior,
        "GrantStealthBehavior",
        "StartRadius = 12\nFinalRadius = 120\nRadiusGrowRate = 3\nEnd\n",
    )
    .unwrap();
    let data = data
        .as_any()
        .downcast_ref::<GrantStealthBehaviorModuleData>()
        .unwrap();
    assert_eq!(
        (data.start_radius, data.final_radius, data.radius_grow_rate),
        (12.0, 120.0, 3.0)
    );
    for body in [
        "UnknownStealthField = 1\nEnd\n",
        "StartRadius = invalid\nEnd\n",
        "StartRadius = 12\n",
    ] {
        assert!(
            parse(
                &mut factory,
                ModuleType::Behavior,
                "GrantStealthBehavior",
                body
            )
            .is_err(),
            "{body}"
        );
    }
}

#[test]
fn hive_and_undead_body_data_share_one_end_with_inherited_health_fields() {
    // HiveStructureBody.h:30 and UndeadBody.cpp:17 append inherited tables.
    let factory = catalog();
    for (name, derived) in [
        (
            "HiveStructureBody",
            "PropagateDamageTypesToSlavesWhenExisting = ALL",
        ),
        ("UndeadBody", "SecondLifeMaxHealth = 234.5"),
    ] {
        let body =
            format!("MaxHealth = 400.5\n{derived}\nInitialHealth = 100.25\nEnd\nFollowingBody\n");
        INI::new()
            .with_inline_source(&body, |ini| {
                let data = factory
                    .construct(Some(ini), name, ModuleType::Behavior)
                    .unwrap();
                let active = if name == "HiveStructureBody" {
                    let hive = data
                        .as_any()
                        .downcast_ref::<HiveStructureBodyModuleData>()
                        .unwrap();
                    assert!(!hive.damage_types_to_propagate_to_slaves.is_empty());
                    &hive.base.base
                } else {
                    let undead = data
                        .as_any()
                        .downcast_ref::<UndeadBodyModuleData>()
                        .unwrap();
                    assert_eq!(undead.second_life_max_health, 234.5);
                    &undead.base
                };
                assert_eq!((active.max_health, active.initial_health), (400.5, 100.25));
                ini.read_line()?;
                assert_eq!(ini.get_line_tokens(), ["FollowingBody"], "{name}");
                Ok(())
            })
            .unwrap();
    }
}

#[test]
fn inherited_body_data_rejects_unknown_invalid_and_truncated_fields() {
    let mut factory = catalog();
    for name in ["HiveStructureBody", "UndeadBody"] {
        for body in [
            "MaxHealth = invalid\nEnd\n",
            "UnknownBodyField = 1\nEnd\n",
            "MaxHealth = 20\n",
        ] {
            assert!(
                parse(&mut factory, ModuleType::Behavior, name, body).is_err(),
                "{name}: {body}"
            );
        }
        assert!(
            parse(&mut factory, ModuleType::Behavior, name, "End").is_ok(),
            "{name}"
        );
    }
}
