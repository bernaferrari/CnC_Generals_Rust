//! Ownership controls for the optional entity-module preview, not gameplay ticking.
//! CPP Object.cpp299–384 helper conditions/order and625–639 module destruction.
use super::*;
use crate::object::helper::ObjectRepulsorHelper;
use crate::world::entities::{TemplateRef, Transform};

fn full_spec() -> EntityModuleInstallSpec {
    EntityModuleInstallSpec {
        template_module_tags: vec!["AuthoredBody".into(), "AuthoredAI".into()],
        can_be_repulsed: true,
        has_weapons: true,
        ..EntityModuleInstallSpec::default()
    }
}

fn install(world: &mut GameWorld) -> EntityId {
    let id = world.spawn_entity(
        TemplateRef::new("PreviewOwner"),
        None,
        Transform::default(),
        100.0,
    );
    world.install_entity_modules(id, &full_spec());
    id
}

fn with_repulsor<R>(
    world: &mut GameWorld,
    id: EntityId,
    f: impl FnOnce(&mut ObjectRepulsorHelper) -> R,
) -> R {
    let helper = world
        .entity_modules
        .live
        .get_mut(&id.get())
        .unwrap()
        .iter_mut()
        .find_map(|module| match module {
            EntityLiveModule::Repulsor(helper) => Some(helper),
            _ => None,
        })
        .expect("actual installed helper");
    f(helper)
}

#[test]
fn preview_same_ids_keep_separate_mutable_helpers() {
    let mut first = GameWorld::new(1);
    let mut second = GameWorld::new(1);
    let a = install(&mut first);
    let b = install(&mut second);
    assert_eq!(a.get(), b.get());
    with_repulsor(&mut first, a, |helper| helper.wake_for_clear(7));
    with_repulsor(&mut second, b, |helper| helper.wake_for_clear(13));
    assert!(with_repulsor(&mut first, a, |helper| helper.should_clear(7)));
    assert!(!with_repulsor(&mut second, b, |helper| helper.should_clear(7)));
    with_repulsor(&mut first, a, |helper| helper.mark_cleared());
    assert!(!with_repulsor(&mut first, a, |helper| helper.needs_clearing()));
    assert!(with_repulsor(&mut second, b, |helper| helper.needs_clearing()));
}

#[test]
fn preview_delete_and_reinstall_preserve_order_and_start_inert() {
    let mut world = GameWorld::new(1);
    let id = install(&mut world);
    let expected = vec![
        HELPER_TAG_SMC,
        HELPER_TAG_STATUS,
        HELPER_TAG_SUBDUAL,
        HELPER_TAG_REPULSOR,
        HELPER_TAG_DEFECTION,
        HELPER_TAG_WEAPON_STATUS,
        HELPER_TAG_FIRING_TRACKER,
        HELPER_TAG_TEMP_WEAPON_BONUS,
        "AuthoredBody",
        "AuthoredAI",
    ];
    assert_eq!(world.entity_module_tags(id), expected.as_slice());
    assert_eq!(world.entity_live_module_count(id), 10);
    with_repulsor(&mut world, id, |helper| helper.wake_for_clear(8));
    assert_eq!(world.walk_entity_modules_on_delete(id), expected);
    assert_eq!(world.entity_live_module_count(id), 0);
    assert_eq!(world.last_entity_on_delete_order(id), expected.as_slice());
    world.install_entity_modules(id, &full_spec());
    assert_eq!(world.entity_module_tags(id), expected.as_slice());
    assert!(!with_repulsor(&mut world, id, |helper| helper.needs_clearing()));
    assert!(!world.entity(id).unwrap().destroyed);
}

#[test]
fn preview_world_reset_discards_only_its_own_instances() {
    let mut first = GameWorld::new(1);
    let mut second = GameWorld::new(1);
    let a = install(&mut first);
    let b = install(&mut second);
    with_repulsor(&mut first, a, |helper| helper.wake_for_clear(7));
    with_repulsor(&mut second, b, |helper| helper.wake_for_clear(13));
    first.clear_entities();
    assert_eq!(first.entity_live_module_count(a), 0);
    assert!(first.entity_module_tags(a).is_empty());
    assert_eq!(second.entity_live_module_count(b), 10);
    assert!(with_repulsor(&mut second, b, |helper| helper.should_clear(13)));
    let replacement = install(&mut first);
    assert!(!with_repulsor(&mut first, replacement, |helper| helper.needs_clearing()));
}
