//! Exact Common authored-definition identity across calling-thread key domains.
//!
//! C++ Locomotor.cpp:594-613 and NameKeyGenerator.cpp:99-146 use one name
//! namespace. These fixtures use actual parse/add/find APIs and owned catalogs;
//! they do not reset the global generator/catalog or claim whole-match isolation.
use super::*;
use std::collections::HashMap;

fn authored(name: &str, speed: &str) -> LocomotorTemplate {
    parse_locomotor_template_definition(
        name,
        &HashMap::from([
            ("Speed".to_string(), speed.to_string()),
            ("Surfaces".to_string(), "GROUND".to_string()),
            ("Appearance".to_string(), "TREADS".to_string()),
        ]),
    )
    .expect("real authored locomotor definition")
}

fn fresh_catalog() -> LocomotorStore {
    std::thread::spawn(|| {
        let mut catalog = LocomotorStore::new();
        catalog
            .add_template(authored("__IdentityHuman", "20"))
            .unwrap();
        catalog
            .add_template(authored("__IdentityJetTaxi", "50"))
            .unwrap();
        assert_eq!(
            catalog
                .find_template("__IdentityHuman")
                .unwrap()
                .name
                .as_str(),
            "__IdentityHuman"
        );
        assert_eq!(
            catalog
                .find_template("__IdentityJetTaxi")
                .unwrap()
                .name
                .as_str(),
            "__IdentityJetTaxi"
        );
        catalog
    })
    .join()
    .expect("authoring thread")
}

#[test]
fn owned_locomotor_catalog_unknown_name_is_absent_across_threads() {
    let catalog = std::sync::Arc::new(fresh_catalog());
    let reader = std::sync::Arc::clone(&catalog);
    std::thread::spawn(move || {
        assert!(
            reader.find_template("__IdentityAbsent").is_none(),
            "a fresh calling-thread key must not alias a loaded locomotor"
        );
        assert_eq!(
            reader
                .find_template("__IdentityHuman")
                .unwrap()
                .name
                .as_str(),
            "__IdentityHuman"
        );
        assert!(
            reader.find_template("__identityhuman").is_none(),
            "CPP NAMEKEY/strcmp keeps exact case identity"
        );
    })
    .join()
    .expect("query thread");
    assert_eq!(catalog.get_template_names().len(), 2);
}

#[test]
fn owned_locomotor_catalog_known_name_selects_its_authored_speed_across_threads() {
    let catalog = fresh_catalog();
    std::thread::spawn(move || {
        let taxi = catalog
            .find_template("__IdentityJetTaxi")
            .expect("loaded taxi");
        assert_eq!(
            taxi.name.as_str(),
            "__IdentityJetTaxi",
            "the actual definition must match the requested name before using its speed"
        );
        assert!((taxi.max_speed * 30.0 - 50.0).abs() < 0.001);
        let human = catalog
            .find_template("__IdentityHuman")
            .expect("loaded human");
        assert_eq!(human.name.as_str(), "__IdentityHuman");
        assert!((human.max_speed * 30.0 - 20.0).abs() < 0.001);
    })
    .join()
    .expect("query thread");
}

#[test]
fn owned_locomotor_catalog_cross_thread_registration_never_replaces_other_name() {
    let catalog = fresh_catalog();
    let catalog = std::thread::spawn(move || {
        let mut catalog = catalog;
        catalog
            .add_template(authored("__IdentityNewJet", "75"))
            .unwrap();
        assert_eq!(
            catalog.get_template_names().len(),
            3,
            "a new name cannot replace an unrelated existing numeric key"
        );
        let taxi = catalog
            .find_template_mut("__IdentityJetTaxi")
            .expect("loaded taxi");
        assert_eq!(taxi.name.as_str(), "__IdentityJetTaxi");
        taxi.max_speed = 60.0 / 30.0;
        catalog
    })
    .join()
    .expect("registration thread");
    std::thread::spawn(move || {
        for (name, speed) in [
            ("__IdentityHuman", 20.0),
            ("__IdentityJetTaxi", 60.0),
            ("__IdentityNewJet", 75.0),
        ] {
            let actual = catalog.find_template(name).expect("retained exact name");
            assert_eq!(actual.name.as_str(), name);
            assert!((actual.max_speed * 30.0 - speed).abs() < 0.001);
        }
    })
    .join()
    .expect("continued query thread");
}

#[test]
fn owned_locomotor_catalog_preallocated_keys_preserve_traversal_and_override_order() {
    std::thread::spawn(|| {
        // C++ map traversal is by the shared key, not name or insertion order.
        // Beta is keyed before its definition, Gamma is defined first.
        NameKeyGenerator::name_to_key("__OrderBeta");
        NameKeyGenerator::name_to_key("__UnrelatedEarlierName");
        let mut catalog = LocomotorStore::new();
        for (name, speed) in [
            ("__OrderGamma", "30"),
            ("__OrderBeta", "20"),
            ("__OrderAlpha", "10"),
        ] {
            catalog.add_template(authored(name, speed)).unwrap();
        }
        let names = || {
            catalog
                .get_template_names()
                .into_iter()
                .map(|name| name.as_str().to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(), vec!["__OrderBeta", "__OrderGamma", "__OrderAlpha"]);
        catalog.add_template(authored("__OrderBeta", "45")).unwrap();
        assert_eq!(
            catalog
                .get_template_names()
                .into_iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            vec!["__OrderBeta", "__OrderGamma", "__OrderAlpha"],
            "replacing a named definition retains its original map position"
        );
        assert!(
            (catalog.find_template("__OrderBeta").unwrap().max_speed * 30.0 - 45.0).abs() < 0.001
        );
    })
    .join()
    .expect("preallocated-key authoring thread");
}

#[test]
fn owned_locomotor_catalog_case_sensitive_definitions_remain_distinct() {
    std::thread::spawn(|| {
        let mut catalog = LocomotorStore::new();
        catalog.add_template(authored("__CaseLoco", "20")).unwrap();
        catalog.add_template(authored("__caseloco", "50")).unwrap();
        assert_eq!(catalog.get_template_names().len(), 2);
        assert!(
            (catalog.find_template("__CaseLoco").unwrap().max_speed * 30.0 - 20.0).abs() < 0.001
        );
        assert!(
            (catalog.find_template("__caseloco").unwrap().max_speed * 30.0 - 50.0).abs() < 0.001
        );
        assert!(catalog.find_template("__CASELOCO").is_none());
    })
    .join()
    .expect("case-sensitive authoring thread");
}

#[test]
fn owned_locomotor_catalog_queries_keep_name_allocation_side_effects() {
    std::thread::spawn(|| {
        let mut catalog = LocomotorStore::new();
        let first = NameKeyGenerator::name_to_key("__AllocationBeforeQuery");
        assert!(catalog.find_template("__AllocationMissingRead").is_none());
        let after_read = NameKeyGenerator::name_to_key("__AllocationAfterRead");
        assert_eq!(after_read, first + 2);
        assert!(
            catalog
                .find_template_mut("__AllocationMissingWrite")
                .is_none()
        );
        let after_write = NameKeyGenerator::name_to_key("__AllocationAfterWrite");
        assert_eq!(after_write, after_read + 2);
        catalog
            .add_template(authored("__AllocationMissingRead", "30"))
            .unwrap();
        assert_eq!(
            NameKeyGenerator::name_to_key("__AllocationMissingRead"),
            first + 1,
            "admission reuses the earlier query allocation"
        );
        let after_add = NameKeyGenerator::name_to_key("__AllocationAfterAdd");
        assert_eq!(after_add, after_write + 1);
        catalog
            .add_template(authored("__AllocationMissingRead", "40"))
            .unwrap();
        assert_eq!(catalog.get_template_names().len(), 1);
        assert_eq!(
            NameKeyGenerator::name_to_key("__AllocationAfterOverride"),
            after_add + 1,
            "a known-name override allocates no replacement name key"
        );
    })
    .join()
    .expect("allocation-observation thread");
}

#[test]
fn owned_locomotor_catalog_cross_thread_override_retains_order_and_other_definition() {
    let catalog = fresh_catalog();
    std::thread::spawn(move || {
        let mut catalog = catalog;
        for name in ["__ForeignA", "__ForeignB", "__ForeignC"] {
            NameKeyGenerator::name_to_key(name);
        }
        catalog
            .add_template(authored("__IdentityHuman", "35"))
            .unwrap();
        assert_eq!(
            catalog
                .get_template_names()
                .into_iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            vec!["__IdentityHuman", "__IdentityJetTaxi"],
            "an override cannot move the definition into the foreign key domain"
        );
        assert!(
            (catalog.find_template("__IdentityHuman").unwrap().max_speed * 30.0 - 35.0).abs()
                < 0.001
        );
        assert!(
            (catalog
                .find_template("__IdentityJetTaxi")
                .unwrap()
                .max_speed
                * 30.0
                - 50.0)
                .abs()
                < 0.001
        );
    })
    .join()
    .expect("foreign-key override thread");
}
