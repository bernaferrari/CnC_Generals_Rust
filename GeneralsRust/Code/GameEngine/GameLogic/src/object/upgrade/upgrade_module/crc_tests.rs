//! C++ CRC authority: UpgradeModule.cpp:20–29,208–230; BehaviorModule.cpp:14–20;
//! Common/Thing/Module.cpp:58–61,112–118. Each listed derived CRC calls only
//! UpgradeModule::crc, while its save method adds the derived version.

use super::*;
use crate::common::{AsciiString, ObjectID, ObjectStatusMaskType};
use crate::helpers::TheThingFactory;
use crate::object::{ModuleEntry, Object};
use game_engine::common::system::{Snapshotable, xfer_load::XferLoad, xfer_save::XferSave};
use game_engine::common::thing::thing_factory::{ensure_thing_factory_exists, get_thing_factory};
use std::io::Cursor;
use std::sync::{Arc, RwLock};

const ID: ObjectID = 0x7A3E_1001;
const TRIGGER: &str = "Upgrade_CrcBaseOnlyFixture";
// Exact original CPP derived crc methods (none add fields or a version):
// Armor86, MaxHealth73, Stealth47, Radar124, ExperienceScalar68,
// ModelCondition60, WeaponSet38 and WeaponBonus74.
const MODULES: &[&str] = &[
    "ArmorUpgrade",
    "MaxHealthUpgrade",
    "StealthUpgrade",
    "RadarUpgrade",
    "ExperienceScalarUpgrade",
    "ModelConditionUpgrade",
    "WeaponSetUpgrade",
    "WeaponBonusUpgrade",
];

#[cfg(not(target_arch = "wasm32"))]
fn child(name: &str) -> bool {
    matches!(
        crate::test_process::run_bounded(
            name.strip_prefix("gamelogic::").unwrap_or(name),
            "GENERALS_UPGRADE_CRC_CHILD",
        ),
        crate::test_process::TestProcess::Child
    )
}
#[cfg(target_arch = "wasm32")]
fn child(_: &str) -> bool {
    true
}

fn bytes(entry: &ModuleEntry, crc: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    entry.with_module(|module| {
        let mut xfer = XferSave::new(Cursor::new(&mut bytes), 1);
        if crc {
            module.crc(&mut xfer).unwrap();
        } else {
            module.xfer(&mut xfer).unwrap();
        }
    });
    bytes
}

fn installed(name: &str) -> (Arc<RwLock<Object>>, Vec<Arc<ModuleEntry>>) {
    assert!(ensure_thing_factory_exists());
    crate::upgrade::center::with_upgrade_center_mut(|center| {
        center.new_upgrade(AsciiString::from(TRIGGER));
    });
    if game_engine::common::thing::module_factory::get_module_factory()
        .unwrap()
        .is_none()
    {
        game_engine::common::thing::module_factory::init_module_factory().unwrap();
    }
    crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
    let behaviors = MODULES
        .iter()
        .enumerate()
        .map(|(index, kind)| {
            format!(" Behavior = {kind} CrcFixture{index}\n TriggeredBy = {TRIGGER}\n End\n")
        })
        .collect::<String>();
    assert_eq!(
        get_thing_factory()
            .unwrap()
            .as_mut()
            .unwrap()
            .load_ini_text(&format!("Object {name}\n KindOf = INERT\n{behaviors}End\n")),
        1
    );
    let template = TheThingFactory::find_template(name).expect("actual authored CRC Object");
    let owner = Arc::new(RwLock::new(Object::new_raw(
        template.clone(),
        ID,
        ObjectStatusMaskType::NONE,
        None,
    )));
    Object::init_modules_for(&owner, template.as_ref()).unwrap();
    let entries = MODULES
        .iter()
        .map(|kind| {
            owner
                .read()
                .unwrap()
                .find_module_by_name(kind)
                .expect("canonical installed CRC module")
        })
        .collect();
    (owner, entries)
}

#[test]
fn base_crc_writes_mux_state_without_save_layer_versions() {
    for executed in [false, true] {
        let mut crc = Vec::new();
        crc_upgrade_module_state(&mut XferSave::new(Cursor::new(&mut crc), 1), executed).unwrap();
        assert_eq!(
            crc,
            vec![1, u8::from(executed)],
            "CPP empty CRC bases contribute no save versions"
        );
        let mut save = Vec::new();
        let mut state = executed;
        xfer_upgrade_module_state(&mut XferSave::new(Cursor::new(&mut save), 1), &mut state)
            .unwrap();
        assert_eq!(
            save,
            vec![1, 1, 1, 1, 1, u8::from(executed)],
            "existing save base envelope is preserved"
        );
    }
}

#[test]
fn authored_base_only_crc_modules_serialize_one_installed_execution_flag() {
    if !child(concat!(
        module_path!(),
        "::authored_base_only_crc_modules_serialize_one_installed_execution_flag"
    )) {
        return;
    }
    let _guard = crate::test_sync::lock();
    // MaxHealth/Armor use their separately staged owned leaf fixes; no ArmorStore readiness or body mutation.
    let (_owner_a, a) = installed("UpgradeCrcInstalledA");
    let (_owner_b, b) = installed("UpgradeCrcInstalledB");
    for ((kind, a), b) in MODULES.iter().zip(&a).zip(&b) {
        assert!(!Arc::ptr_eq(a, b), "{kind}: independent installed entry");
        assert_eq!(bytes(a, true), vec![1, 0], "{kind}: inert constructor CRC");
        assert_eq!(
            bytes(a, false),
            vec![1, 1, 1, 1, 1, 1, 0],
            "{kind}: derived save version is retained"
        );
        a.with_module(|module| {
            module
                .xfer(&mut XferLoad::new(
                    Cursor::new(vec![1, 1, 1, 1, 1, 1, 1]),
                    1,
                ))
                .unwrap()
        });
        assert_eq!(
            bytes(a, true),
            vec![1, 1],
            "{kind}: CRC follows this installed flag"
        );
        assert_eq!(
            bytes(a, false),
            vec![1, 1, 1, 1, 1, 1, 1],
            "{kind}: save restores this installed flag"
        );
        assert_eq!(
            bytes(b, true),
            vec![1, 0],
            "{kind}: another same-ID entry is unchanged"
        );
        assert_eq!(
            bytes(b, false),
            vec![1, 1, 1, 1, 1, 1, 0],
            "{kind}: restore stays local"
        );
    }
}
