//! C++ Weapon.cpp:437-462: authored range * RANGE bonus, then one quarter-cell deduction.
//! Exercise real parsed rules -> normal GameLogic admission -> live Object range query.
use super::*;
use crate::game_logic::{GameLogic, Team};
use glam::Vec3;

struct RestoreInputs {
    store: Option<gamelogic::weapon::WeaponStore>,
    authority: crate::game_logic::game_logic::GameWorldAuthority,
    frame: u32,
}

impl RestoreInputs {
    fn install() -> Self {
        let saved = Self {
            store: gamelogic::weapon::with_weapon_store_mut(std::mem::take).ok(),
            authority: crate::game_logic::game_logic::current_gameworld_authority(),
            frame: crate::game_logic::host_historic_bonus::logic_frame(),
        };
        crate::game_logic::game_logic::gameworld_authority::publish_gameworld_authority(
            crate::game_logic::game_logic::GameWorldAuthority::DEFAULT_OFF,
        );
        // A prior fixture may remove the store while the independent seed flag survives.
        gamelogic::weapon::initialize_weapon_store().expect("actual native WeaponStore");
        crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
        assert_eq!(
            crate::assets::ini_template_loader::register_weapons_from_ini_text(
                r#"
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
"#
            ),
            3
        );
        gamelogic::weapon::with_weapon_store(|store| {
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
        })
        .expect("inspect actual rules");
        saved
    }
}

impl Drop for RestoreInputs {
    fn drop(&mut self) {
        if let Some(previous) = self.store.take() {
            gamelogic::weapon::with_weapon_store_mut(|store| *store = previous)
                .expect("restore prior native store");
        } else {
            gamelogic::weapon::shutdown_weapon_store().expect("restore absent store");
        }
        crate::game_logic::game_logic::gameworld_authority::publish_gameworld_authority(
            self.authority,
        );
        crate::game_logic::host_historic_bonus::set_logic_frame(self.frame);
    }
}

fn admit(name: &str) -> (GameLogic, crate::game_logic::ObjectId) {
    let mut logic = GameLogic::new();
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
    let _serial = crate::game_logic::combat::tests::combat_test_guard();
    let _restore = RestoreInputs::install();
    let expected = gamelogic::weapon::with_weapon_store(|store| {
        store
            .find_weapon_template("AuthoredRangeShort")
            .unwrap()
            .get_attack_range(&gamelogic::weapon::WeaponBonus::new())
    })
    .unwrap();
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
    let _serial = crate::game_logic::combat::tests::combat_test_guard();
    let _restore = RestoreInputs::install();
    let (maximum, minimum) = gamelogic::weapon::with_weapon_store(|store| {
        let rules = store.find_weapon_template("AuthoredRangeMinimum").unwrap();
        (
            rules.get_attack_range(&gamelogic::weapon::WeaponBonus::new()),
            rules.get_minimum_attack_range(),
        )
    })
    .unwrap();
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
    let _serial = crate::game_logic::combat::tests::combat_test_guard();
    let _restore = RestoreInputs::install();
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
    let expected = gamelogic::weapon::with_weapon_store(|store| {
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
    })
    .unwrap();
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
    let _serial = crate::game_logic::combat::tests::combat_test_guard();
    let _restore = RestoreInputs::install();
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
    let _serial = crate::game_logic::combat::tests::combat_test_guard();
    let _restore = RestoreInputs::install();
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
    let victim = logic.objects.get_mut(&target).unwrap();
    victim.set_orientation(0.0);
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
    logic
        .objects
        .get_mut(&target)
        .unwrap()
        .set_position(Vec3::new(9.48, 0.0, 0.0));
    assert!(
        !logic.objects[&source].can_pursue_target(&logic.objects[&target]),
        "below the native minimum remains too close to pursue"
    );
}

#[path = "authored_weapon_range_reached_reader_tests.rs"]
mod reached_reader_tests;
