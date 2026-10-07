//! Weapon.cpp:2189-2205 accepts a contact attack on a structure only when
//! the partition collision iterator contains that actual victim.
use super::*;

fn contact_pair() -> (FactoryRuntime, Arc<RwLock<Object>>) {
    definitions();
    assert_eq!(
        get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(
            "Object OwnedContactStructure\n KindOf = STRUCTURE IMMOBILE\n Geometry = SPHERE\n GeometryMajorRadius = 5\n GeometryHeight = 5\nEnd\n"
        ),
        1
    );
    let mut actual = FactoryRuntime::new();
    let position = *actual.owner.read().unwrap().get_position();
    let victim_id = actual
        ._factory
        .create_object(
            "OwnedContactStructure",
            position,
            None,
            ObjectCreationFlags::empty(),
        )
        .unwrap();
    let victim = actual
        ._factory
        .get_object(victim_id)
        .unwrap()
        .get_base_object()
        .unwrap();
    assert!(
        victim
            .read()
            .unwrap()
            .is_kind_of(crate::common::KindOf::Structure)
    );
    assert!(Arc::ptr_eq(
        &victim,
        &TheGameLogic::find_object_by_id(victim_id).unwrap()
    ));
    assert!(super::super::registry::get_unit_arc(victim_id).is_none());
    {
        let mut owner = actual.owner.write().unwrap();
        let mut template = crate::weapon::WeaponTemplate::new("OwnedContactWeapon".into());
        template.attack_range = 1.0;
        template.primary_damage = 10.0;
        let mut weapons = crate::weapon::WeaponTemplateSet::new();
        weapons.set_weapon_template(crate::weapon::WeaponSlotType::Primary, Arc::new(template));
        owner.weapon_set.add_weapon_template_set(weapons);
        owner.refresh_weapon_set().unwrap();
        owner.reload_all_ammo(true).unwrap();
        assert!(owner.get_current_weapon().unwrap().0.is_contact_weapon());
    }
    // Perform normal partition maintenance for the genuinely admitted factory
    // objects before taking any object guards. Never fabricate a Unit handle.
    crate::system::game_logic::get_game_logic()
        .lock()
        .unwrap()
        .partition_manager_mut()
        .update()
        .unwrap();
    let owner = actual.owner.read().unwrap();
    let hits = crate::helpers::ThePartitionManager::get()
        .unwrap()
        .iterate_potential_collisions(owner.get_position(), owner.get_geometry_info(), 0.0);
    assert!(hits.contains(&actual.id));
    assert!(
        hits.contains(&victim_id),
        "actual victim must be a collision candidate"
    );
    drop(owner);
    (actual, victim)
}

fn borrowed_contact(actual: &FactoryRuntime, victim: &Arc<RwLock<Object>>) -> bool {
    let _held_ai = actual.ai.lock().unwrap();
    let owner = actual.owner.write().unwrap();
    let target = victim.write().unwrap();
    owner
        .get_current_weapon()
        .unwrap()
        .0
        .is_within_attack_range_for_objects(&owner, Some(&target), None)
}

#[test]
fn factory_contact_uses_held_source_and_victim() {
    if !child(concat!(
        module_path!(),
        "::factory_contact_uses_held_source_and_victim"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let (actual, victim) = contact_pair();
    assert!(borrowed_contact(&actual, &victim));
}

#[test]
fn factory_contact_ignores_locked_same_id_registry_decoys() {
    if !child(concat!(
        module_path!(),
        "::factory_contact_ignores_locked_same_id_registry_decoys"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let (actual, victim) = contact_pair();
    let victim_id = victim.read().unwrap().get_id();
    let source_decoy = Arc::new(RwLock::new(Object::new_test(actual.id, 100.0)));
    let victim_decoy = Arc::new(RwLock::new(Object::new_test(victim_id, 100.0)));
    for (id, decoy) in [(actual.id, &source_decoy), (victim_id, &victim_decoy)] {
        decoy
            .write()
            .unwrap()
            .set_position(&Coord3D::new(500.0, 600.0, 0.0))
            .unwrap();
        crate::object::registry::OBJECT_REGISTRY.register_object(id, decoy);
    }
    let _held_source_decoy = source_decoy.write().unwrap();
    let _held_victim_decoy = victim_decoy.write().unwrap();
    assert!(borrowed_contact(&actual, &victim));
}

#[test]
fn factory_contact_does_not_invent_partition_membership() {
    if !child(concat!(
        module_path!(),
        "::factory_contact_does_not_invent_partition_membership"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let (actual, victim) = contact_pair();
    let id = victim.read().unwrap().get_id();
    crate::system::game_logic::get_game_logic()
        .lock()
        .unwrap()
        .partition_manager_mut()
        .remove_object(id);
    assert!(!borrowed_contact(&actual, &victim));
}

#[test]
fn factory_contact_still_filters_live_victim_outside_collision_radius() {
    if !child(concat!(
        module_path!(),
        "::factory_contact_still_filters_live_victim_outside_collision_radius"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let (actual, victim) = contact_pair();
    let owner = actual.owner.read().unwrap();
    let mut pos = *owner.get_position();
    let radius = owner
        .get_geometry_info()
        .get_bounding_circle_radius()
        .max(1.0);
    pos.x += radius + 0.5;
    drop(owner);
    // Keep the candidate from the preceding partition update, but move its
    // live position beyond the collision circle. The structure's 5wu radius
    // still makes the preliminary weapon boundary-distance check succeed.
    victim.write().unwrap().set_position(&pos).unwrap();
    assert!(!borrowed_contact(&actual, &victim));
}
