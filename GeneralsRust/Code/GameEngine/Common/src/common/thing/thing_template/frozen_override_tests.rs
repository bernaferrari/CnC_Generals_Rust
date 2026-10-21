use super::*;
use crate::common::ini::INILoadType;
use crate::common::name_key_generator::NameKeyNamespace;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn identified(mut template: ThingTemplate, id: u16, name: &str, cost: u16) -> ThingTemplate {
    template.set_template_id(id);
    template.set_template_name(name.into());
    template.set_build_cost(cost);
    template
}

#[test]
fn receiving_constructor_has_no_compatibility_override_slot() {
    let frozen = Arc::new(ThingTemplate::new_receiving());
    assert!(matches!(
        &frozen.next_override,
        TemplateOverrideLink::Frozen
    ));
    assert!(frozen.get_next_override().is_none());
    assert!(Arc::ptr_eq(
        &ThingTemplate::get_final_override(&frozen),
        &frozen
    ));
    assert!(frozen.clone().has_frozen_override_link());
    assert!(!ThingTemplate::new().has_frozen_override_link());
}

#[test]
fn compatibility_override_slots_preserve_shared_clone_and_final_identity() {
    // Overridable.h:31-49: actual compatibility aliases observe the same chain.
    let base = Arc::new(identified(ThingTemplate::new(), 11, "Base", 100));
    let query = Arc::new(base.as_ref().clone());
    let first = Arc::new(identified(ThingTemplate::new(), 11, "First", 200));
    let final_version = Arc::new(identified(ThingTemplate::new(), 11, "Final", 300));
    base.set_next_override(Some(Arc::clone(&first)));
    first.set_next_override(Some(Arc::clone(&final_version)));
    assert!(Arc::ptr_eq(&query.get_next_override().unwrap(), &first));
    assert!(Arc::ptr_eq(
        &ThingTemplate::get_final_override(&base),
        &final_version
    ));
    assert!(Arc::ptr_eq(
        &ThingTemplate::get_final_override(&query),
        &final_version
    ));
    assert!(base.is_equivalent_to(final_version.as_ref()));
    query.delete_overrides();
    assert!(base.get_next_override().is_none());
    assert!(Arc::ptr_eq(
        &ThingTemplate::get_final_override(&base),
        &base
    ));
    assert!(Arc::ptr_eq(
        &first.get_next_override().unwrap(),
        &final_version
    ));
    Overridable::delete_overrides(first.as_ref());
    assert!(first.get_next_override().is_none());
}

#[test]
fn copy_from_preserves_destination_override_mode_and_cpp_identity() {
    // ThingTemplate.cpp:1195-1210 preserves this destination's name/id/list.
    let master_next = Arc::new(ThingTemplate::new());
    let successor = Arc::new(ThingTemplate::new());
    let mut compatible = identified(ThingTemplate::new(), 17, "Compatible", 100);
    compatible.set_next_template(Some(Arc::clone(&master_next)));
    compatible.mark_as_override();
    compatible.set_next_override(Some(Arc::clone(&successor)));
    let frozen_source = identified(ThingTemplate::new_receiving(), 29, "FrozenSource", 700);
    compatible.copy_from(&frozen_source);
    assert!(!compatible.has_frozen_override_link());
    assert_eq!(compatible.get_template_id(), 17);
    assert_eq!(compatible.get_name().as_str(), "Compatible");
    assert_eq!(compatible.get_build_cost(), 700);
    assert!(compatible.is_override());
    assert!(Arc::ptr_eq(
        compatible.get_next_template().as_ref().unwrap(),
        &master_next
    ));
    assert!(Arc::ptr_eq(
        &compatible.get_next_override().unwrap(),
        &successor
    ));
    let mut frozen = identified(ThingTemplate::new_receiving(), 41, "Frozen", 200);
    frozen.set_next_template(Some(Arc::clone(&master_next)));
    frozen.mark_as_override();
    frozen.copy_from(&compatible);
    assert!(frozen.has_frozen_override_link());
    assert!(frozen.get_next_override().is_none());
    assert_eq!(frozen.get_template_id(), 41);
    assert_eq!(frozen.get_name().as_str(), "Frozen");
    assert_eq!(frozen.get_build_cost(), 700);
    assert!(frozen.is_override());
    assert!(Arc::ptr_eq(
        frozen.get_next_template().as_ref().unwrap(),
        &master_next
    ));
    assert!(Arc::ptr_eq(
        &compatible.get_next_override().unwrap(),
        &successor
    ));
}

#[test]
fn frozen_override_mutations_reject_without_changing_definition_or_peer() {
    let frozen = identified(ThingTemplate::new_receiving(), 53, "Immutable", 900);
    let peer = frozen.clone();
    let successor = Arc::new(ThingTemplate::new());
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            frozen.set_next_override(Some(Arc::clone(&successor)));
        }))
        .is_err()
    );
    assert!(catch_unwind(AssertUnwindSafe(|| frozen.set_next_override(None))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| frozen.delete_overrides())).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| Overridable::delete_overrides(&frozen))).is_err());
    for value in [&frozen, &peer] {
        assert!(value.has_frozen_override_link());
        assert!(value.get_next_override().is_none());
        assert_eq!(value.get_template_id(), 53);
        assert_eq!(value.get_name().as_str(), "Immutable");
        assert_eq!(value.get_build_cost(), 900);
    }
}

#[test]
fn receiving_admission_detaches_empty_seeds_and_rejects_linked_compatibility_inputs() {
    let seed = identified(ThingTemplate::new(), 67, "Seed", 1100);
    let fields = HashMap::new();
    let mut modules = ModuleFactory::new_receiving(NameKeyNamespace::new());
    let admitted = seed
        .admit_named_receiving_module_source(
            "Admitted",
            &fields,
            INILoadType::Overwrite,
            ReceivingTemplateKind::Object,
            &mut modules,
        )
        .expect("explicit empty compatibility seed can be detached");
    assert!(admitted.template().has_frozen_override_link());
    assert_eq!(admitted.template_id(), 67);
    assert_eq!(admitted.name(), "Admitted");
    assert_eq!(admitted.template().get_build_cost(), 1100);
    assert!(!seed.has_frozen_override_link());
    assert_eq!(seed.get_name().as_str(), "Seed");
    assert!(seed.get_next_override().is_none());
    let validation = seed
        .prepare_named_receiving_candidate(
            "Validated",
            &fields,
            INILoadType::Overwrite,
            ReceivingTemplateKind::Object,
            &modules,
        )
        .expect("same detached validation boundary");
    assert_eq!(validation.summary().name, "Validated");
    assert!(
        AdmittedReceivingTemplate::from_parsed_template(ThingTemplate::new()).is_err(),
        "publication must not retain a compatibility slot"
    );
    let successor = Arc::new(ThingTemplate::new());
    seed.set_next_override(Some(Arc::clone(&successor)));
    let error = seed
        .admit_named_receiving_module_source(
            "WrongOwner",
            &fields,
            INILoadType::Overwrite,
            ReceivingTemplateKind::Object,
            &mut modules,
        )
        .expect_err("caller must explicitly select a definition before admission");
    assert!(error.contains("explicitly selected definition"), "{error}");
    assert!(
        seed.prepare_named_receiving_candidate(
            "WrongValidation",
            &fields,
            INILoadType::Overwrite,
            ReceivingTemplateKind::Object,
            &modules,
        )
        .is_err()
    );
    assert!(Arc::ptr_eq(&seed.get_next_override().unwrap(), &successor));
    assert!(admitted.template().has_frozen_override_link());
    assert_eq!(admitted.name(), "Admitted");
}
