use super::*;
use crate::common::NameKeyType;
use crate::helpers::TheFXListStore;

#[test]
fn fx_catalog_does_not_alias_names_from_another_key_namespace() {
    std::thread::spawn(|| {
        NameKeyGenerator::init();
        TheFXListStore::ensure_fx_list("FX_CatalogNamespaceExisting");
    })
    .join()
    .unwrap();

    std::thread::spawn(|| {
        NameKeyGenerator::init();
        // The missing name receives the same numeric key as the previous
        // thread's existing name. C++ findFXList still compares exact names.
        assert!(TheFXListStore::find_fx_list("FX_CatalogNamespaceMissing").is_none());
        let fx = TheFXListStore::find_fx_list("FX_CatalogNamespaceExisting").unwrap();
        assert_eq!(fx.name(), "FX_CatalogNamespaceExisting");
    })
    .join()
    .unwrap();
}

#[test]
fn owned_fx_catalogs_are_inert_and_isolated() {
    std::thread::spawn(|| {
        use game_engine::common::ini::ini_fx_list::{FXList as Definition, FXListStore};
        NameKeyGenerator::init();
        let mut first = FXListStore::new();
        let mut second = FXListStore::new();
        first.add_fx_list(Definition::new("FX_OwnedCatalogFirst".into()));
        second.add_fx_list(Definition::new("FX_OwnedCatalogSecond".into()));
        assert!(first.find_fx_list("FX_OwnedCatalogSecond").is_none());
        assert!(second.find_fx_list("FX_OwnedCatalogFirst").is_none());
        assert!(TheFXListStore::find_fx_list("FX_OwnedCatalogFirst").is_none());
        assert_eq!(NameKeyGenerator::name_to_key("FirstAmbientKey"), 1);

        let definition = first.find_fx_list("FX_OwnedCatalogFirst").unwrap();
        let buffer = definition.name.as_str().as_ptr();
        let retained = FXList::from_authored_name(&definition.name);
        assert_eq!(
            retained.name().as_ptr(),
            buffer,
            "binding shares immutable authored name"
        );
        first = FXListStore::new();
        assert!(first.find_fx_list(retained.name()).is_none());
        assert_eq!(retained.name(), "FX_OwnedCatalogFirst");
        assert_eq!(
            retained.name().as_ptr(),
            buffer,
            "reference survives catalog reset"
        );
        assert!(second.find_fx_list("FX_OwnedCatalogSecond").is_some());
    })
    .join()
    .unwrap();
}

#[test]
fn owned_fx_catalog_keeps_exact_case_and_null_sentinel() {
    use game_engine::common::ini::ini_fx_list::{FXList as Definition, FXListStore};
    let mut store = FXListStore::new();
    store.add_fx_list(Definition::new("FX_CaseSensitive".into()));
    store.add_fx_list(Definition::new("fx_casesensitive".into()));
    assert_eq!(
        store
            .find_fx_list("FX_CaseSensitive")
            .unwrap()
            .name
            .as_str(),
        "FX_CaseSensitive"
    );
    assert_eq!(
        store
            .find_fx_list("fx_casesensitive")
            .unwrap()
            .name
            .as_str(),
        "fx_casesensitive"
    );
    assert!(store.find_fx_list("FX_CASESENSITIVE").is_none());
    store.add_fx_list(Definition::new("None".into()));
    assert!(store.find_fx_list("None").is_none());
    assert!(store.find_fx_list("nOnE").is_none());
}

#[test]
fn fx_reference_resolves_id_in_the_dispatching_namespace() {
    let fx = std::thread::spawn(|| {
        NameKeyGenerator::init();
        FXList::new("FX_DispatchNamespaceReference")
    })
    .join()
    .unwrap();

    std::thread::spawn(move || {
        NameKeyGenerator::init();
        NameKeyGenerator::name_to_key("UnrelatedDispatchKey");
        assert_eq!(
            NameKeyGenerator::key_to_name(fx.id() as NameKeyType).as_deref(),
            Some("FX_DispatchNamespaceReference")
        );
    })
    .join()
    .unwrap();
}
