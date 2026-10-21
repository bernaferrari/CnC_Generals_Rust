//! Actual registered parser, CPP parseScience identity, and receiving isolation.
use crate::object::create::veterancy_gain_create::VeterancyGainCreateModuleData;
use game_engine::common::{
    ini::INI,
    name_key_generator::NameKeyNamespace,
    rts::science::{ScienceInfo, ScienceNameFacts, ScienceStore},
    thing::{
        module::{ModuleData, ModuleType},
        module_factory::ModuleFactory,
    },
};
use std::sync::Arc;
fn catalog(rows: &[(&str, i32)]) -> ModuleFactory {
    let mut store = ScienceStore::new();
    for &(name, id) in rows {
        store.add_science(ScienceInfo::new(id, name));
    }
    let mut f = ModuleFactory::new_receiving_with_sciences(
        NameKeyNamespace::new(),
        Arc::new(ScienceNameFacts::from_store(&store)),
    );
    crate::contain_module_overrides::register_native_module_overrides(&mut f).unwrap();
    f
}
fn parse(f: &mut ModuleFactory, ini: &mut INI, text: &str) -> Result<Arc<dyn ModuleData>, String> {
    ini.with_inline_source(text, |ini| {
        Ok(f.new_module_data_from_ini(
            Some(ini),
            "VeterancyGainCreate",
            ModuleType::Behavior,
            "VeterancyTag",
        ))
    })
    .unwrap()
}
fn science(data: &Arc<dyn ModuleData>) -> i32 {
    data.as_any()
        .downcast_ref::<VeterancyGainCreateModuleData>()
        .expect("actual registered Veterancy data")
        .science_required
}
#[test]
#[cfg(not(target_arch = "wasm32"))]
fn registered_veterancy_science_uses_its_receiving_catalog_with_foreign_store_locked() {
    let name = concat!(
        module_path!(),
        "::registered_veterancy_science_uses_its_receiving_catalog_with_foreign_store_locked"
    );
    let name = name.split_once("::").unwrap().1;
    if matches!(
        crate::test_process::run_bounded(name, "GENERALS_VETERANCY_SCIENCE_OWNER_CHILD"),
        crate::test_process::TestProcess::ParentVerified
    ) {
        return;
    }
    let mut a = catalog(&[("SCIENCE_RedGuardTraining", 701)]);
    let mut b = catalog(&[("SCIENCE_RedGuardTraining", 902)]);
    let mut empty = catalog(&[]);
    game_engine::common::rts::science::init_science_store();
    let mut foreign = game_engine::common::rts::science::get_science_store_mut().unwrap();
    foreign.add_science(ScienceInfo::new(33, "SCIENCE_RedGuardTraining"));
    let mut ini = INI::new();
    let text = "StartingLevel = ELITE\nScienceRequired = SCIENCE_RedGuardTraining\nEnd\n";
    let first = parse(&mut a, &mut ini, text).unwrap();
    let peer = parse(&mut b, &mut ini, text).unwrap();
    assert_eq!(
        science(&first),
        701,
        "receiving science ID must not come from foreign application store"
    );
    assert_eq!(
        science(&peer),
        902,
        "same name in another receiving catalog keeps its own ID"
    );
    assert!(
        parse(&mut empty, &mut ini, text).is_err(),
        "Some(empty) cannot fall back to the locked foreign store"
    );
    assert!(
        parse(&mut a, &mut ini, "ScienceRequired = SCIENCE_Unknown\nEnd\n").is_err(),
        "CPP friend_lookupScience rejects unknown names"
    );
    let retained = parse(&mut a, &mut ini, text).unwrap();
    assert_eq!(science(&retained), 701);
    drop(b);
    assert_eq!(
        science(&first),
        701,
        "peer disposal cannot rename retained module data"
    );
    drop(foreign);
    // The same INI has left its receiving operation, including after error.
    let mut compatibility = VeterancyGainCreateModuleData::default();
    ini.with_inline_source(text, |ini| compatibility.parse_from_ini(ini))
        .unwrap();
    assert_eq!(
        compatibility.science_required, 33,
        "standalone compatibility remains explicit after scoped loan"
    );
}
#[test]
fn registered_veterancy_accepts_all_five_genuine_retail_science_names_without_global_facts() {
    let names = [
        "Infa_SCIENCE_RedGuardTraining",
        "SCIENCE_ArtilleryTraining",
        "SCIENCE_BattlemasterTraining",
        "SCIENCE_RedGuardTraining",
        "SCIENCE_TechnicalTraining",
    ];
    let rows = names
        .iter()
        .enumerate()
        .map(|(i, n)| (*n, 1001 + i as i32))
        .collect::<Vec<_>>();
    let mut factory = catalog(&rows);
    let mut ini = INI::new();
    for (name, id) in rows {
        let data = parse(
            &mut factory,
            &mut ini,
            &format!("ScienceRequired = {name}\nEnd\n"),
        )
        .unwrap();
        assert_eq!(
            science(&data),
            id,
            "actual registered parser must retain the provided application Science ID for {name}"
        );
    }
    assert!(parse(&mut factory, &mut ini, "ScienceRequired =\nEnd\n").is_err());
}
