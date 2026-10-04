//! Real Object INI projection -> ordinary Main Ranger creation.
//! CPP WeaponSet.cpp:279–320 allocates only authored getNth(slot).
use super::*;
use crate::game_logic::GameLogic;

fn parsed_ranger(name: &str, weapon_set: &str) -> ThingTemplate {
    let source = format!(
        "Object {name}\n  Type = Infantry\n  KindOf = INFANTRY SELECTABLE ATTACKABLE\n  Body = ActiveBody ModuleTag_Body\n    MaxHealth = 100\n  End\n{weapon_set}End\n"
    );
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(&source, "ranger_primary_admission.ini")
        .expect("real Object INI parser");
    let definition = parser
        .get_definition(name)
        .expect("parsed Ranger definition");
    assert!(crate::game_logic::host_ranger::is_ranger_template(name));
    let template = GameLogic::build_template_from_object_definition(name, definition, None);
    assert!(template.is_kind_of(KindOf::Infantry));
    assert!(template.is_kind_of(KindOf::Attackable));
    assert_eq!(template.max_health, 100.0);
    template
}

fn ordinary_create(template: ThingTemplate) -> (GameLogic, crate::game_logic::ObjectId) {
    let name = template.name.clone();
    let mut world = GameLogic::new();
    world.templates.insert(name.clone(), template);
    let id = world
        .create_object(&name, Team::USA, Vec3::ZERO)
        .expect("ordinary Main creation of actual projected definition");
    assert_eq!(world.host_object(id).unwrap().health.current, 100.0);
    (world, id)
}

#[test]
fn ranger_primary_admission_authored_none_stays_unarmed() {
    isolated(
        "ranger_primary_admission_tests::ranger_primary_admission_authored_none_stays_unarmed",
        || {
            let _restore = RestoreStore::install();
            let template = parsed_ranger(
                "AuthoredPrimaryNoneRanger",
                "  WeaponSet\n    Conditions = None\n    Weapon = PRIMARY None\n  End\n",
            );
            assert!(
                template.primary_weapon_explicitly_none,
                "projection retains authored None"
            );
            assert!(
                !template.primary_auto_choose_none,
                "this witness isolates PRIMARY None"
            );
            assert!(template.primary_weapon_name.is_none());
            assert!(template.primary_weapon.is_none());
            assert!(
                template.resolve_primary_weapon().is_none(),
                "resolution is already correctly unarmed"
            );
            let (world, id) = ordinary_create(template);
            let object = world.host_object(id).unwrap();
            assert!(object.thing.template.primary_weapon_explicitly_none);
            assert!(object.thing.template.primary_weapon_name.is_none());
            assert!(object.thing.template.primary_weapon.is_none());
            assert!(object.secondary_weapon.is_none());
            assert!(object.tertiary_weapon.is_none());
            assert!(
                object.weapon.is_none(),
                "actual creation must not re-arm authored PRIMARY None via Ranger name fallback"
            );
            assert!(!object.can_attack());
        },
    );
}

#[test]
fn ranger_primary_admission_named_auto_choose_none_keeps_real_weapon() {
    isolated(
        "ranger_primary_admission_tests::ranger_primary_admission_named_auto_choose_none_keeps_real_weapon",
        || {
            let _restore = RestoreStore::install();
            const RULE: &str = "AuthoredPrimaryRangerRule";
            assert_eq!(
                crate::assets::ini_template_loader::register_weapons_from_ini_text(
                    "Weapon AuthoredPrimaryRangerRule\n  PrimaryDamage = 37\n  AttackRange = 123\n  DelayBetweenShots = 1000\n  WeaponSpeed = 30000000\n  ClipSize = 8\n  DamageType = SMALL_ARMS\n  AntiGround = Yes\n  ProjectileObject = NONE\nEnd\n"
                ),
                1
            );
            let template = parsed_ranger(
                "AuthoredPrimaryNamedRanger",
                "  WeaponSet\n    Conditions = None\n    Weapon = PRIMARY AuthoredPrimaryRangerRule\n    AutoChooseSources = PRIMARY NONE\n  End\n",
            );
            assert!(!template.primary_weapon_explicitly_none);
            assert!(
                template.primary_auto_choose_none,
                "autonomous source policy is not absence of a valid weapon"
            );
            assert_eq!(template.primary_weapon_name.as_deref(), Some(RULE));
            assert!(template.primary_weapon.is_none());
            let resolved = template
                .resolve_primary_weapon()
                .expect("real named rule resolves before auto-choose suppression");
            assert_eq!(resolved.damage, 37.0);
            assert_eq!(resolved.range, 123.0);
            let (world, id) = ordinary_create(template);
            let object = world.host_object(id).unwrap();
            assert!(object.thing.template.primary_auto_choose_none);
            assert_eq!(
                object.thing.template.primary_weapon_name.as_deref(),
                Some(RULE)
            );
            let weapon = object
                .weapon
                .as_ref()
                .expect("actual authored primary retained");
            assert_eq!(weapon.damage, 37.0);
            assert_eq!(weapon.range, 123.0);
            assert_eq!(weapon.ammo, Some(8));
            assert!(object.can_attack());
        },
    );
}

#[test]
fn ranger_primary_admission_missing_rule_keeps_existing_fallback() {
    isolated(
        "ranger_primary_admission_tests::ranger_primary_admission_missing_rule_keeps_existing_fallback",
        || {
            let _restore = RestoreStore::install();
            let template = parsed_ranger("MissingPrimaryRanger", "");
            assert!(!template.primary_weapon_explicitly_none);
            assert!(!template.primary_auto_choose_none);
            assert!(template.primary_weapon_name.is_none());
            assert!(template.primary_weapon.is_none());
            let (world, id) = ordinary_create(template);
            let object = world.host_object(id).unwrap();
            let weapon = object
                .weapon
                .as_ref()
                .expect("existing missing-authoring host fallback");
            assert_eq!(weapon.damage, 5.0);
            assert_eq!(weapon.range, 100.0);
            assert!(object.can_attack());
        },
    );
}
