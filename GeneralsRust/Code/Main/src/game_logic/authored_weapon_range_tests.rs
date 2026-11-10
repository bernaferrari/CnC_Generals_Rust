//! C++ Weapon.cpp:437-462: authored range * RANGE bonus, then one quarter-cell deduction.
//! Exercise real parsed rules -> normal GameLogic admission -> live Object range query.
use super::*;
use crate::game_logic::{GameLogic, Team};
use glam::Vec3;

const AUTHORED_RANGE_WEAPONS: &str = r#"
Weapon AuthoredRangeShort
  PrimaryDamage = 11
  AttackRange = 5
  MinimumAttackRange = 0
End
Weapon AuthoredRangeMinimum
  PrimaryDamage = 12
  AttackRange = 100
  MinimumAttackRange = 10
End
Weapon AuthoredRangeBonus
  PrimaryDamage = 13
  AttackRange = 100
  MinimumAttackRange = 0
  WeaponBonus = PLAYER_UPGRADE RANGE 200%
End
"#;

const AUTHORED_SLOT_WEAPONS: &str = r#"
Weapon AuthoredSlotRangeA
  PrimaryDamage = 13
  AttackRange = 100
  WeaponBonus = PLAYER_UPGRADE RANGE 50%
End
Weapon AuthoredSlotRangeB
  PrimaryDamage = 17
  AttackRange = 100
  WeaponBonus = PLAYER_UPGRADE RANGE 300%
End
"#;

/// Authored Weapon rules parsed through the ordinary Weapon.ini loader and
/// owned by the receiving world (C++ loads TheWeaponStore before any Object,
/// GameEngine.cpp:428-458); no ambient store is installed or restored.
pub(super) struct AuthoredRules(Vec<(String, gamelogic::weapon::WeaponTemplate)>);

impl AuthoredRules {
    pub(super) fn parse() -> Self {
        let mut rows = std::collections::HashMap::new();
        assert_eq!(
            crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
                &mut rows,
                &format!("{AUTHORED_RANGE_WEAPONS}{AUTHORED_SLOT_WEAPONS}"),
                crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
            ),
            5
        );
        Self(rows.into_values().map(|rule| (rule.name.clone(), rule)).collect())
    }

    pub(super) fn find_weapon_template(
        &self,
        name: &str,
    ) -> Option<&gamelogic::weapon::WeaponTemplate> {
        self.0.iter().find(|(row, _)| row == name).map(|(_, rule)| rule)
    }

    /// Admit the rules to `logic` before any Object exists.
    pub(super) fn admit(self, logic: &mut GameLogic) {
        logic
            .admit_receiving_weapon_snapshot(self.0)
            .expect("authored Weapon content admitted before Objects");
    }
}

#[test]
fn authored_rules_parse_exact_named_rows() {
    let store = AuthoredRules::parse();
    for (name, damage, range, min) in [
        ("AuthoredRangeShort", 11.0, 5.0, 0.0),
        ("AuthoredRangeMinimum", 12.0, 100.0, 10.0),
        ("AuthoredRangeBonus", 13.0, 100.0, 0.0),
    ] {
        let rules = store
            .find_weapon_template(name)
            .expect("parsed exact named rules");
        assert_eq!(rules.primary_damage, damage);
        assert_eq!(rules.attack_range, range);
        assert_eq!(rules.minimum_attack_range, min);
    }
}

fn admit(name: &str) -> (GameLogic, crate::game_logic::ObjectId) {
    let mut logic = GameLogic::new();
    AuthoredRules::parse().admit(&mut logic);
    let mut template = ThingTemplate::new("AuthoredRangeShooter");
    template.set_primary_weapon_name(name);
    template.set_health(100.0);
    template
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable);
    assert!(
        template.primary_weapon.is_none(),
        "normal admission must resolve the actual named rules"
    );
    logic.templates.insert(template.name.clone(), template);
    let id = logic
        .create_object("AuthoredRangeShooter", Team::USA, Vec3::ZERO)
        .expect("actual GameLogic roster admission");
    assert_eq!(
        logic.objects[&id]
            .thing()
            .template
            .primary_weapon_name
            .as_deref(),
        Some(name)
    );
    (logic, id)
}

#[test]
fn authored_short_range_survives_one_runtime_deduction() {
    let expected = {
        let store = &AuthoredRules::parse();
        store
            .find_weapon_template("AuthoredRangeShort")
            .unwrap()
            .get_attack_range(&gamelogic::weapon::WeaponBonus::new())
    };
    assert_eq!(expected, 2.5, "unchanged native C++ yardstick");
    let (logic, id) = admit("AuthoredRangeShort");
    let object = &logic.objects[&id];
    assert_eq!(object.weapon_slot(0).unwrap().damage, 11.0);
    assert_eq!(
        object
            .thing()
            .template
            .geometry_info
            .bounding_circle_radius(),
        1.0
    );
    let mine_position = Vec3::new(2.0, 0.0, 0.0);
    assert_eq!(object.distance_to_pos(mine_position), 1.0);
    assert!(
        object.is_within_attack_range_pos_for_slot(0, mine_position),
        "authored 5 has effective range 2.5; the mine at contact distance 1 is in range"
    );
    assert!(object.is_within_attack_range_at_distance(0, expected));
    assert!(!object.is_within_attack_range_at_distance(0, expected + 0.01));
    assert_eq!(object.weapon_slot(0).unwrap().range, 5.0);
}

#[test]
fn authored_minimum_range_is_rationalized_once_at_runtime() {
    let (maximum, minimum) = {
        let store = &AuthoredRules::parse();
        let rules = store.find_weapon_template("AuthoredRangeMinimum").unwrap();
        (
            rules.get_attack_range(&gamelogic::weapon::WeaponBonus::new()),
            rules.get_minimum_attack_range(),
        )
    };
    assert_eq!(
        (maximum, minimum),
        (97.5, 7.5),
        "unchanged native C++ yardsticks"
    );
    let (logic, id) = admit("AuthoredRangeMinimum");
    let object = &logic.objects[&id];
    assert_eq!(object.weapon_slot(0).unwrap().damage, 12.0);
    assert!(
        !object.is_within_attack_range_at_distance(0, minimum - 0.01),
        "the native minimum boundary cannot be deducted a second time"
    );
    assert!(object.is_within_attack_range_at_distance(0, minimum));
    assert!(object.is_within_attack_range_at_distance(0, maximum));
    assert!(!object.is_within_attack_range_at_distance(0, maximum + 0.01));
    assert_eq!(
        (
            object.weapon_slot(0).unwrap().range,
            object.weapon_slot(0).unwrap().min_range
        ),
        (100.0, 10.0)
    );
}

#[test]
fn authored_range_bonus_precedes_one_runtime_deduction() {
    let (mut logic, id) = admit("AuthoredRangeBonus");
    logic
        .objects
        .get_mut(&id)
        .unwrap()
        .weapon_bonus_player_upgrade = true;
    let object = &logic.objects[&id];
    assert_eq!(object.weapon_slot(0).unwrap().damage, 13.0);
    assert_eq!(
        object.weapon_bonus_fields().1,
        2.0,
        "actual parsed extra RANGE 200% is active"
    );
    let expected = {
        let store = &AuthoredRules::parse();
        let rules = store.find_weapon_template("AuthoredRangeBonus").unwrap();
        let authored_bonus = rules
            .extra_bonus
            .as_ref()
            .unwrap()
            .get_bonus(gamelogic::weapon::WeaponBonusConditionType::PlayerUpgrade)
            .unwrap();
        assert_eq!(
            authored_bonus.get_field(gamelogic::weapon::WeaponBonusField::Range),
            2.0
        );
        rules.get_attack_range(authored_bonus)
    };
    assert_eq!(expected, 197.5, "100 * 2 - 2.5, not (100 - 2.5) * 2 - 2.5");
    assert!(
        object.is_within_attack_range_at_distance(0, expected),
        "the quarter-cell deduction must not be multiplied by RANGE bonus"
    );
    assert!(!object.is_within_attack_range_at_distance(0, expected + 0.01));
    assert_eq!(object.weapon_slot(0).unwrap().range, 100.0);
}

#[test]
fn explicit_raw_weapon_admission_keeps_existing_runtime_boundaries() {
    let mut logic = GameLogic::new();
    let mut template = ThingTemplate::new("ExplicitRawRangeShooter");
    template.set_primary_weapon(Weapon {
        damage: 14.0,
        range: 100.0,
        min_range: 10.0,
        ..Weapon::default()
    });
    template.set_health(100.0);
    template
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable);
    logic.templates.insert(template.name.clone(), template);
    let id = logic
        .create_object("ExplicitRawRangeShooter", Team::USA, Vec3::ZERO)
        .unwrap();
    let object = &logic.objects[&id];
    assert_eq!(object.weapon_slot(0).unwrap().damage, 14.0);
    assert_eq!(
        (
            object.weapon_slot(0).unwrap().range,
            object.weapon_slot(0).unwrap().min_range
        ),
        (100.0, 10.0)
    );
    assert!(!object.is_within_attack_range_at_distance(0, 7.49));
    assert!(object.is_within_attack_range_at_distance(0, 7.5));
    assert!(object.is_within_attack_range_at_distance(0, 97.5));
    assert!(!object.is_within_attack_range_at_distance(0, 97.51));
}

#[test]
fn named_weapon_pursuit_keeps_native_minimum_boundary() {
    let (mut logic, source) = admit("AuthoredRangeMinimum");
    let mut template = ThingTemplate::new("AuthoredRangeMovingVictim");
    template.set_health(100.0);
    template
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable);
    logic.templates.insert(template.name.clone(), template);
    let target = logic
        .create_object(
            "AuthoredRangeMovingVictim",
            Team::GLA,
            Vec3::new(9.5, 0.0, 0.0),
        )
        .unwrap();
    logic.objects.get_mut(&source).unwrap().movement.max_speed = 10.0;
    let (victim, fixture_partition_victim) = logic.host_object_and_partition_mut(target).unwrap();
    victim.set_orientation_with_partition_owner(0.0, fixture_partition_victim);
    victim.movement.velocity = Vec3::new(5.0, 0.0, 0.0);
    let source_object = &logic.objects[&source];
    let victim = &logic.objects[&target];
    assert_eq!(source_object.effective_max_speed(), 10.0);
    assert_eq!(victim.forward_speed_2d(), 5.0);
    assert_eq!(source_object.distance_to_object(victim), 7.5);
    assert!(
        source_object.can_pursue_target(victim),
        "C++ isTooClose admits exact rationalized minimum 7.5"
    );
    logic.set_fixture_object_position(target, Vec3::new(9.48, 0.0, 0.0));
    assert!(
        !logic.objects[&source].can_pursue_target(&logic.objects[&target]),
        "below the native minimum remains too close to pursue"
    );
}

#[path = "authored_weapon_range_reached_reader_tests.rs"]
mod reached_reader_tests;
