//! Real named A/B rules, normal admission, and primary acquisition while B remains selected.
use super::*;

fn slot_range_world(
    active_slot: u8,
    victim_distance: f32,
) -> (
    GameLogic,
    crate::game_logic::ObjectId,
    crate::game_logic::ObjectId,
) {
    assert_eq!(
        crate::assets::ini_template_loader::register_weapons_from_ini_text(
            r#"
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
"#
        ),
        2
    );
    gamelogic::weapon::with_weapon_store(|store| {
        for (name, multiplier, native_range) in [
            ("AuthoredSlotRangeA", 0.5, 47.5),
            ("AuthoredSlotRangeB", 3.0, 297.5),
        ] {
            let rules = store.find_weapon_template(name).unwrap();
            let extra = rules
                .extra_bonus
                .as_ref()
                .unwrap()
                .get_bonus(gamelogic::weapon::WeaponBonusConditionType::PlayerUpgrade)
                .unwrap();
            assert_eq!(
                extra.get_field(gamelogic::weapon::WeaponBonusField::Range),
                multiplier
            );
            assert_eq!(rules.get_attack_range(extra), native_range);
        }
    })
    .unwrap();
    let mut logic = GameLogic::new();
    let mut defense = ThingTemplate::new("AuthoredSlotRangeDefense");
    defense.set_health(100.0);
    defense
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBaseDefense)
        .add_kind_of(KindOf::Attackable);
    defense.set_primary_weapon_name("AuthoredSlotRangeA");
    defense.set_secondary_weapon_name("AuthoredSlotRangeB");
    defense.geometry_info.major_radius = 0.0;
    defense.geometry_info.authored = true;
    assert!(
        defense.primary_weapon.is_none() && defense.secondary_weapon.is_none(),
        "actual named admission resolves both rules"
    );
    logic.templates.insert(defense.name.clone(), defense);
    let mut victim = ThingTemplate::new("AuthoredSlotRangeVictim");
    victim.set_health(100.0);
    victim
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable);
    victim.geometry_info.major_radius = 0.0;
    victim.geometry_info.authored = true;
    logic.templates.insert(victim.name.clone(), victim);
    let source = logic
        .create_object("AuthoredSlotRangeDefense", Team::USA, Vec3::ZERO)
        .unwrap();
    let victim = logic
        .create_object(
            "AuthoredSlotRangeVictim",
            Team::GLA,
            Vec3::new(victim_distance, 0.0, 0.0),
        )
        .unwrap();
    let object = logic.objects.get_mut(&source).unwrap();
    object.weapon_bonus_player_upgrade = true;
    object.set_active_weapon_slot(active_slot);
    assert_eq!(object.weapon_name_for_slot(0), Some("AuthoredSlotRangeA"));
    assert_eq!(object.weapon_name_for_slot(1), Some("AuthoredSlotRangeB"));
    assert_eq!(object.weapon_slot(0).unwrap().damage, 13.0);
    assert_eq!(object.weapon_slot(1).unwrap().damage, 17.0);
    assert_eq!(object.weapon_slot(0).unwrap().range, 100.0);
    assert_eq!(object.weapon_slot(1).unwrap().range, 100.0);
    assert!(
        !crate::game_logic::host_base_defense::is_dual_slot_base_defense(&object.template_name),
        "generic FSBaseDefense actually fires primary0"
    );
    assert!(!object.turret_enabled);
    assert_eq!(object.active_weapon_slot, active_slot);
    assert_eq!(
        object.weapon_bonus_fields().1,
        if active_slot == 0 { 0.5 } else { 3.0 }
    );
    assert_eq!(
        logic.objects[&source].distance_to_object(&logic.objects[&victim]),
        victim_distance
    );
    logic.frame = 300;
    assert!(crate::game_logic::Object::weapon_ready(
        logic.objects[&source].weapon_slot(0).unwrap(),
        10.0
    ));
    (logic, source, victim)
}

#[test]
fn custom_authored_primary_range_uses_its_parsed_bonus_at_exact_boundary() {
    let _serial = crate::game_logic::combat::tests::combat_test_guard();
    let _restore = RestoreInputs::install();
    let (mut logic, source, victim) = slot_range_world(0, 47.5);
    assert_eq!(
        logic.objects[&source].weapon_bonus_fields_for_slot(0).1,
        0.5
    );
    assert_eq!(
        logic.objects[&source].weapon_bonus_fields_for_slot(1).1,
        3.0
    );
    logic.update_combat(&[source], 1.0 / 30.0);
    assert_eq!(
        logic.objects[&victim].health.current, 87.0,
        "actual primary0 damage at native A range47.5"
    );
    assert_eq!(logic.objects[&source].active_weapon_slot, 0);
}

#[test]
fn custom_authored_primary_acquisition_does_not_use_selected_secondary_bonus() {
    let _serial = crate::game_logic::combat::tests::combat_test_guard();
    let _restore = RestoreInputs::install();
    let (mut logic, source, victim) = slot_range_world(1, 60.0);
    assert_eq!(
        logic.objects[&source].weapon_bonus_fields_for_slot(0).1,
        0.5
    );
    assert_eq!(
        logic.objects[&source].weapon_bonus_fields_for_slot(1).1,
        3.0
    );
    logic.update_combat(&[source], 1.0 / 30.0);
    assert_eq!(
        logic.objects[&victim].health.current, 100.0,
        "60 is beyond A47.5 even though selected B permits297.5"
    );
    assert_eq!(
        logic.objects[&source].active_weapon_slot, 1,
        "query must not retask the selected slot"
    );
    assert_eq!(
        logic.objects[&source].weapon_bonus_fields().1,
        3.0,
        "legacy active query still selects B extras"
    );
}

#[test]
fn unnamed_secondary_range_has_no_primary_extra_but_legacy_fallback_is_kept() {
    let _serial = crate::game_logic::combat::tests::combat_test_guard();
    let _restore = RestoreInputs::install();
    // Register the genuine A/B rows through the same live parser/admission seam.
    let (_registered_world, _, _) = slot_range_world(0, 47.5);
    let mut logic = GameLogic::new();
    let mut definition = ThingTemplate::new("AuthoredUnnamedSecondaryRange");
    definition.set_primary_weapon_name("AuthoredSlotRangeA");
    definition.set_secondary_weapon(Weapon {
        range: 100.0,
        damage: 17.0,
        ..Weapon::default()
    });
    definition
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBaseDefense)
        .add_kind_of(KindOf::Attackable);
    logic.templates.insert(definition.name.clone(), definition);
    let id = logic
        .create_object("AuthoredUnnamedSecondaryRange", Team::USA, Vec3::ZERO)
        .unwrap();
    let object = logic.objects.get_mut(&id).unwrap();
    object.weapon_bonus_player_upgrade = true;
    object.set_active_weapon_slot(1);
    assert!(object.weapon_slot(1).is_some());
    assert_eq!(object.weapon_name_for_slot(1), None);
    assert_eq!(
        object.weapon_bonus_fields().1,
        0.5,
        "existing public active query retains primary fallback"
    );
    assert_eq!(object.weapon_bonus_fields_for_slot(0).1, 0.5);
    assert_eq!(
        object.weapon_bonus_fields_for_slot(1).1,
        1.0,
        "unnamed secondary has only common condition bonuses"
    );
    assert_eq!(object.effective_weapon_range_for_slot(1, 100.0), 100.0);
    assert_eq!(object.active_weapon_slot, 1);
}
