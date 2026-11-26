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
    {
        let store = &AuthoredRules::parse();
        for (name, multiplier, native_range) in [
            ("AuthoredSlotRangeA", 0.5, 47.5),
            ("AuthoredSlotRangeB", 3.0, 297.5),
        ] {
            let rules = store.find_weapon_template(name).unwrap();
            let extra = rules
                .extra_bonus
                .as_ref()
                .unwrap()
                .get_bonus(generals_sim::weapon::WeaponBonusConditionType::PlayerUpgrade)
                .unwrap();
            assert_eq!(
                extra.get_field(generals_sim::weapon::WeaponBonusField::Range),
                multiplier
            );
            assert_eq!(rules.get_attack_range(extra), native_range);
        }
    }
    let mut logic = GameLogic::new();
    AuthoredRules::parse().admit(&mut logic);
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
    assert!(!object.turret_ai.turret_enabled);
    assert_eq!(object.active_weapon_slot, active_slot);
    assert_eq!(
        logic
            .receiving_weapon_bonus_fields_for_slot(&logic.objects[&source], active_slot)
            .1,
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
    let (mut logic, source, victim) = slot_range_world(0, 47.5);
    // The firing weapon's own extra set comes from the receiving world's Weapon rules.
    for (slot, expected) in [(0, 0.5), (1, 3.0)] {
        assert_eq!(
            logic
                .receiving_weapon_bonus_fields_for_slot(&logic.objects[&source], slot)
                .1,
            expected
        );
    }
    logic.update_combat(&[source], 1.0 / 30.0);
    assert_eq!(
        logic.objects[&victim].health.current, 87.0,
        "actual primary0 damage at native A range47.5"
    );
    assert_eq!(logic.objects[&source].active_weapon_slot, 0);
}

#[test]
fn custom_authored_primary_acquisition_does_not_use_selected_secondary_bonus() {
    let (mut logic, source, victim) = slot_range_world(1, 60.0);
    // The firing weapon's own extra set comes from the receiving world's Weapon rules.
    for (slot, expected) in [(0, 0.5), (1, 3.0)] {
        assert_eq!(
            logic
                .receiving_weapon_bonus_fields_for_slot(&logic.objects[&source], slot)
                .1,
            expected
        );
    }
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
        logic
            .receiving_weapon_bonus_fields_for_slot(&logic.objects[&source], 1)
            .1,
        3.0,
        "the selected slot still reads B's extras"
    );
}

#[test]
fn unnamed_secondary_has_no_slot_and_no_primary_extra() {
    // C++ WeaponSet.cpp: an Object has only the slots its authored WeaponSet names,
    // and each slot's extra bonuses come from that slot's own weapon.
    let mut logic = GameLogic::new();
    AuthoredRules::parse().admit(&mut logic);
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[crate::fixture_content::FixtureObject::new("AuthoredUnnamedSecondaryRange")
            .kind_of("STRUCTURE FS_BASE_DEFENSE")
            .health(100.0)
            .armed("AuthoredSlotRangeA")],
    );
    let id = logic
        .create_object("AuthoredUnnamedSecondaryRange", Team::USA, Vec3::ZERO)
        .unwrap();
    let object = logic.objects.get_mut(&id).unwrap();
    object.weapon_bonus_player_upgrade = true;
    assert!(object.weapon_slot(0).is_some());
    assert!(object.weapon_slot(1).is_none(), "no SECONDARY in the WeaponSet");
    assert_eq!(object.authored_weapon_name_for_slot(1), None);
    let object = &logic.objects[&id];
    assert_eq!(logic.receiving_weapon_bonus_fields_for_slot(object, 0).1, 0.5);
    assert_eq!(
        logic.receiving_weapon_bonus_fields_for_slot(object, 1).1,
        1.0,
        "unnamed secondary has only common condition bonuses"
    );
}
