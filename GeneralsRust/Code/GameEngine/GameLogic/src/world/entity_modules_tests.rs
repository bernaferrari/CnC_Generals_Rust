use super::*;
use crate::world::entities::{TemplateRef, Transform};

#[test]
fn default_helpers_then_template_and_on_delete_preserves_order() {
    let mut world = GameWorld::new(1);
    let id = world.spawn_entity(
        TemplateRef::new("TestObject"),
        None,
        Transform::default(),
        10.0,
    );
    let spec = EntityModuleInstallSpec {
        template_module_tags: vec!["BodyModule".to_string(), "AIUpdate".to_string()],
        ..EntityModuleInstallSpec::default()
    };
    let tags = world.install_entity_modules(id, &spec);
    assert_eq!(
        tags,
        vec![
            HELPER_TAG_SMC,
            HELPER_TAG_STATUS,
            HELPER_TAG_SUBDUAL,
            HELPER_TAG_DEFECTION,
            "BodyModule",
            "AIUpdate",
        ]
    );
    assert_eq!(world.entity_module_tags(id), tags.as_slice());
    let installed = world
        .entity(id)
        .and_then(|e| e.entity_modules.clone())
        .expect("entity modules");
    assert!(installed.on_created);
    assert_eq!(installed.records.len(), tags.len());
    assert_eq!(installed.records[0].handle, "ObjectSMCHelper");
    assert_eq!(installed.live_instances, tags.len());
    assert_eq!(world.entity_live_module_count(id), tags.len());
    let deleted = world.walk_entity_modules_on_delete(id);
    assert_eq!(deleted, tags);
    assert_eq!(world.last_entity_on_delete_order(id), tags.as_slice());
    assert!(world.entity_module_tags(id).is_empty());
}

#[test]
fn tank_fixture_installs_helpers_then_template_in_cpp_order() {
    let spec = EntityModuleInstallSpec {
        template_module_tags: vec!["ActiveBody".to_string(), "AIUpdate".to_string()],
        has_weapons: true,
        ..EntityModuleInstallSpec::default()
    };
    let mut expected = ctor_helper_tags(&spec);
    expected.extend(spec.template_module_tags.iter().cloned());
    assert_eq!(
        expected,
        vec![
            HELPER_TAG_SMC,
            HELPER_TAG_STATUS,
            HELPER_TAG_SUBDUAL,
            HELPER_TAG_DEFECTION,
            HELPER_TAG_WEAPON_STATUS,
            HELPER_TAG_FIRING_TRACKER,
            HELPER_TAG_TEMP_WEAPON_BONUS,
            "ActiveBody",
            "AIUpdate",
        ]
    );
    let mut world = GameWorld::new(1);
    let id = world.spawn_entity(
        TemplateRef::new("AmericaTankCrusader"),
        None,
        Transform::default(),
        100.0,
    );
    let installed = world.install_entity_modules(id, &spec);
    assert_eq!(installed, expected);
    world.mark_entity_destroyed(id);
    assert_eq!(world.process_destroy_list(), 1);
    assert_eq!(world.last_entity_on_delete_order(id), expected.as_slice());
}

#[test]
fn shrubbery_omits_defection_inactive_body_omits_status() {
    let spec = EntityModuleInstallSpec {
        inactive_body: true,
        shrubbery: true,
        ..EntityModuleInstallSpec::default()
    };
    assert_eq!(ctor_helper_tags(&spec), vec![HELPER_TAG_SMC.to_string()]);
}

#[test]
fn flag_on_install_matches_crate_init_modules_for_tags() {
    let crate_obj = crate::object::Object::new_test(11, 80.0);
    let crate_tags = crate_obj.installed_module_tags();
    assert!(!crate_tags.is_empty());

    let mut world = GameWorld::new(1);
    let id = world.spawn_entity(
        TemplateRef::new("TestObject"),
        None,
        Transform::default(),
        80.0,
    );
    let installed = world.install_entity_modules_from_crate(id, &crate_obj);
    assert_eq!(installed, crate_tags);
    assert_eq!(world.entity_module_tags(id), crate_tags.as_slice());
    assert_eq!(world.entity_live_module_count(id), crate_tags.len());
}
