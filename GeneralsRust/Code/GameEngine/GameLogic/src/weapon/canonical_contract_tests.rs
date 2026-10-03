//! Behavioral assertions retained when the dormant private weapon stack was removed.
//! All instances below use the same types exported to the production WeaponSet.

use super::*;
use game_engine::common::name_key_generator::NameKeyGenerator;

#[test]
fn authored_ranges_apply_cpp_undersize_and_clamp() {
    // Weapon.cpp437–462: both ranges subtract one quarter of a pathfind cell.
    let mut template = WeaponTemplate::new("CanonicalRanges".into());
    template.attack_range = 100.0;
    template.minimum_attack_range = 10.0;
    let mut bonus = WeaponBonus::new();
    assert_eq!(template.get_attack_range(&bonus), 97.5);
    assert_eq!(template.get_minimum_attack_range(), 7.5);
    bonus.set_field(WeaponBonusField::Range, 2.0);
    assert_eq!(template.get_attack_range(&bonus), 197.5);
    template.attack_range = 1.0;
    template.minimum_attack_range = 1.0;
    assert_eq!(template.get_attack_range(&bonus), 0.0);
    assert_eq!(template.get_minimum_attack_range(), 0.0);
}

#[test]
fn authored_damage_and_radius_use_independent_bonuses() {
    // Weapon.cpp507–523 multiplies damage and radius by their respective fields.
    let mut template = WeaponTemplate::new("CanonicalDamage".into());
    template.primary_damage = 50.0;
    template.primary_damage_radius = 25.0;
    let mut bonus = WeaponBonus::new();
    bonus.set_field(WeaponBonusField::Damage, 1.5);
    bonus.set_field(WeaponBonusField::Radius, 2.0);
    assert_eq!(template.get_primary_damage(&bonus), 75.0);
    assert_eq!(template.get_primary_damage_radius(&bonus), 50.0);
}

#[test]
fn authored_contact_and_laser_classification() {
    // Weapon.cpp531–543 / Weapon.h: contact is range-based; laser is a name.
    let mut template = WeaponTemplate::new("CanonicalKinds".into());
    template.attack_range = 5.0;
    assert!(template.is_contact_weapon());
    template.attack_range = 100.0;
    assert!(!template.is_contact_weapon());
    assert!(!template.is_laser());
    template.laser_name = "CanonicalLaser".into();
    assert!(template.is_laser());
}

#[test]
fn authored_timing_uses_rof_division_and_preattack_truncation() {
    let isolation = weapon_range_test_guard();
    let _fixture = ScopedWeaponFixture::new(&isolation);
    let mut template = WeaponTemplate::new("CanonicalTiming".into());
    template.min_delay_between_shots = 10;
    template.max_delay_between_shots = 20;
    template.clip_reload_time = 60;
    template.pre_attack_delay = 15;
    let mut bonus = WeaponBonus::new();
    bonus.set_field(WeaponBonusField::RateOfFire, 2.0);
    bonus.set_field(WeaponBonusField::PreAttack, 0.5);
    let delay = template.get_delay_between_shots(&bonus);
    assert!((5..=10).contains(&delay));
    assert_eq!(template.get_clip_reload_time(&bonus), 30);
    assert_eq!(template.get_pre_attack_delay(&bonus), 7);
    // Weapon.cpp480–483 specifically avoids a random draw for equal bounds.
    template.max_delay_between_shots = template.min_delay_between_shots;
    let seed = game_engine::common::random_value::get_game_logic_random_seed_state();
    assert_eq!(template.get_delay_between_shots(&bonus), 5);
    assert_eq!(
        game_engine::common::random_value::get_game_logic_random_seed_state(),
        seed
    );
}

#[test]
fn override_link_preserves_existing_template_identity() {
    // WeaponStore::newOverride, Weapon.cpp1575–1586, links the prior template.
    let mut template = WeaponTemplate::new("CanonicalOverride".into());
    assert!(!template.is_override());
    template.set_next_template(WeaponTemplate::new("PriorTemplate".into()));
    assert!(template.is_override());
    assert_eq!(template.name, "CanonicalOverride");
}

#[test]
fn omitted_weapon_masks_match_cpp_defaults() {
    let template = WeaponTemplate::new("CanonicalDefaults".into());
    assert_eq!(template.clip_size, 0);
    assert_eq!(
        template.affects_mask.bits(),
        WeaponAffectsMask::ALLIES | WeaponAffectsMask::ENEMIES | WeaponAffectsMask::NEUTRALS
    );
    assert_eq!(template.collide_mask.bits(), WeaponCollideMask::STRUCTURES);
}

/// One authored definition in the existing parser store. Remove exactly this
/// fixture's name, preserving every unrelated definition and its registration order.
struct AuthoredWeaponDefinition {
    name: crate::common::AsciiString,
}

impl AuthoredWeaponDefinition {
    fn new(name: &str, properties: std::collections::HashMap<String, String>) -> Self {
        use game_engine::common::ini::ini_weapon;
        ini_weapon::initialize_weapon_store();
        let name = crate::common::AsciiString::from(name);
        let absent = ini_weapon::get_weapon_store()
            .unwrap()
            .find_template(&name)
            .is_none();
        assert!(
            absent,
            "authored weapon fixture must not replace an existing definition"
        );
        let mut definition = ini_weapon::WeaponTemplate::new(name.clone());
        definition
            .update_from_properties(&properties)
            .expect("parse actual weapon definition");
        let fixture = Self { name };
        ini_weapon::get_weapon_store()
            .unwrap()
            .register_template(definition);
        fixture
    }
}

impl Drop for AuthoredWeaponDefinition {
    fn drop(&mut self) {
        game_engine::common::ini::ini_weapon::get_weapon_store()
            .unwrap()
            .remove_template(&self.name);
    }
}

#[test]
fn authored_radius_damage_affects_replaces_default_in_canonical_import() {
    #[cfg(not(target_arch = "wasm32"))]
    if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
        "weapon::tests::canonical_contract_tests::authored_radius_damage_affects_replaces_default_in_canonical_import",
        "GENERALS_CANONICAL_WEAPON_IMPORT_CHILD",
    ) {
        return;
    }
    let isolation = weapon_range_test_guard();
    let _fixture = ScopedWeaponFixture::new(&isolation);
    let definition = AuthoredWeaponDefinition::new(
        "CanonicalMaskContract",
        std::collections::HashMap::from([(
            "RadiusDamageAffects".into(),
            "ENEMIES NOT_AIRBORNE".into(),
        )]),
    );
    let mut store = WeaponStore::new();
    store
        .init()
        .expect("import actual parsed weapon definition");
    let template = store
        .find_weapon_template(definition.name.as_str())
        .expect("canonical imported template");
    assert_eq!(template.affects_mask.bits() & WeaponAffectsMask::ALLIES, 0);
    assert_eq!(
        template.affects_mask.bits() & WeaponAffectsMask::NEUTRALS,
        0
    );
    assert_ne!(template.affects_mask.bits() & WeaponAffectsMask::ENEMIES, 0);
    assert_ne!(
        template.affects_mask.bits() & WeaponAffectsMask::DOESNT_AFFECT_AIRBORNE,
        0
    );
}

fn assert_missile_collision_uses_kindof(kind: &str, collision_mask: u32) {
    let isolation = weapon_range_test_guard();
    let _fixture = ScopedWeaponFixture::new(&isolation);
    let bodies = AdmittedWeaponBodies::new();
    let source = crate::object::registry::OBJECT_REGISTRY
        .get_object(bodies.source_id())
        .unwrap();
    let target = crate::object::registry::OBJECT_REGISTRY
        .get_object(bodies.target_id())
        .unwrap();
    let mut projectile = crate::common::DefaultThingTemplate::new("PlainProjectile".into());
    projectile.add_kind_of(crate::common::KindOf::Projectile);
    source
        .write()
        .unwrap()
        .set_template_for_test(Arc::new(projectile));
    let mut target_template = crate::common::DefaultThingTemplate::new("PlainTarget".into());
    target_template.parse_object_fields_from_ini(&std::collections::HashMap::from([(
        "KindOf".into(),
        format!("PROJECTILE {kind}"),
    )]));
    target
        .write()
        .unwrap()
        .set_template_for_test(Arc::new(target_template));
    let mut weapon = WeaponTemplate::new("CanonicalMissileCollision".into());
    weapon.collide_mask = WeaponCollideMask::new(collision_mask);
    assert!(weapon.should_projectile_collide_with(
        INVALID_ID,
        bodies.source_id(),
        bodies.target_id(),
        INVALID_ID
    ));
    weapon.collide_mask = WeaponCollideMask::new(WeaponCollideMask::STRUCTURES);
    assert!(!weapon.should_projectile_collide_with(
        INVALID_ID,
        bodies.source_id(),
        bodies.target_id(),
        INVALID_ID
    ));
}

#[test]
fn small_missile_collision_uses_kindof_not_template_name() {
    assert_missile_collision_uses_kindof("SMALL_MISSILE", WeaponCollideMask::SMALL_MISSILES);
}

#[test]
fn ballistic_missile_collision_uses_kindof_not_template_name() {
    assert_missile_collision_uses_kindof(
        "BALLISTIC_MISSILE",
        WeaponCollideMask::BALLISTIC_MISSILES,
    );
}

#[test]
fn canonical_store_counts_and_missing_names() {
    let mut store = WeaponStore::new();
    assert_eq!(store.get_template_count(), 0);
    assert_eq!(store.get_delayed_damage_count(), 0);
    let template = store.add_weapon_template(WeaponTemplate::new("CanonicalStore".into()));
    assert_eq!(store.get_template_count(), 1);
    assert!(Arc::ptr_eq(
        store.find_weapon_template("CanonicalStore").unwrap(),
        &template
    ));
    assert!(store.find_weapon_template("AbsentCanonicalStore").is_none());
}

#[test]
fn canonical_store_none_is_missing_case_insensitively() {
    // Weapon.cpp1536–1543 reserves None even when a same-name template exists.
    let mut store = WeaponStore::new();
    store.add_weapon_template(WeaponTemplate::new("None".into()));
    for name in ["None", "none", "NONE"] {
        assert!(store.find_weapon_template(name).is_none());
    }
}

#[test]
fn canonical_store_assigns_name_key_and_preserves_explicit_key() {
    let mut store = WeaponStore::new();
    let expected_key = NameKeyGenerator::name_to_key("CanonicalComputedKey");
    let computed = store.add_weapon_template(WeaponTemplate::new("CanonicalComputedKey".into()));
    assert_eq!(computed.name_key, expected_key);
    assert!(Arc::ptr_eq(
        store
            .find_weapon_template_by_name_key(expected_key)
            .unwrap(),
        &computed
    ));
    let mut template = WeaponTemplate::new("CanonicalExplicitKey".into());
    template.name_key = 12345;
    let explicit = store.add_weapon_template(template);
    assert_eq!(explicit.name_key, 12345);
    assert!(Arc::ptr_eq(
        store.find_weapon_template_by_name_key(12345).unwrap(),
        &explicit
    ));
}

#[test]
fn canonical_store_allocates_runtime_weapon_with_exact_template() {
    let mut store = WeaponStore::new();
    let template = store.add_weapon_template(WeaponTemplate::new("CanonicalAllocated".into()));
    let weapon = store.allocate_new_weapon(&template, WeaponSlotType::Primary);
    assert_eq!(weapon.get_name(), "CanonicalAllocated");
    assert_eq!(weapon.get_weapon_slot(), WeaponSlotType::Primary);
    assert!(Arc::ptr_eq(weapon.get_template(), &template));
}

#[test]
fn canonical_store_validation_accepts_unlimited_clip_and_rejects_invalid_fields() {
    let mut store = WeaponStore::new();
    let mut valid = WeaponTemplate::new("CanonicalValid".into());
    valid.attack_range = 100.0;
    valid.minimum_attack_range = 10.0;
    valid.clip_size = 0;
    store.add_weapon_template(valid);
    assert!(store.validate_templates().is_ok());
    let mut invalid = WeaponTemplate::new("CanonicalInvalidRange".into());
    invalid.attack_range = 50.0;
    invalid.minimum_attack_range = 100.0;
    store.add_weapon_template(invalid);
    assert!(store.validate_templates().is_err());
    let mut store = WeaponStore::new();
    let mut invalid = WeaponTemplate::new("CanonicalNegativeClip".into());
    invalid.clip_size = -1;
    store.add_weapon_template(invalid);
    assert!(store.validate_templates().is_err());
}

#[test]
fn canonical_delayed_damage_snapshot_retains_queue_identity() {
    let mut store = WeaponStore::new();
    assert!(honesty_weapon_store_delayed_damage_residual_ok(&store));
    assert!(store.delayed_damage_snapshot_residual().is_empty());
    let template = store.add_weapon_template(WeaponTemplate::new("CanonicalDelayed".into()));
    let pos = Coord3D::new(100.0, 50.0, 25.0);
    let bonus = WeaponBonus::new();
    store.set_delayed_damage(&template, &pos, 900, 1, 2, &bonus);
    store.set_delayed_damage_from_template(
        &WeaponTemplate::new("CanonicalDelayedFromRef".into()),
        &Coord3D::new(0.0, 0.0, 0.0),
        901,
        3,
        INVALID_ID,
        &bonus,
    );
    assert_eq!(store.get_delayed_damage_count(), 2);
    assert!(honesty_weapon_store_delayed_damage_residual_ok(&store));
    let entries = store.delayed_damage_snapshot_residual();
    assert_eq!(entries[0].weapon_name, "CanonicalDelayed");
    assert_eq!(entries[0].delay_damage_frame, 900);
    assert_eq!(entries[0].delay_source_id, 1);
    assert_eq!(entries[0].delay_intended_victim_id, 2);
    assert_eq!(entries[0].delay_damage_pos, pos);
    assert_eq!(entries[1].weapon_name, "CanonicalDelayedFromRef");
    assert_eq!(entries[1].delay_damage_frame, 901);
    assert_eq!(entries[1].delay_source_id, 3);
    assert_eq!(entries[1].delay_intended_victim_id, INVALID_ID);
    assert_eq!(entries[1].delay_damage_pos, Coord3D::new(0.0, 0.0, 0.0));
    assert!(
        entries
            .iter()
            .all(WeaponDelayedDamageSnapshotResidual::honesty_ok)
    );
}
