//! Exercise both actual Main consuming roots against a frozen C++ CRC oracle.
use super::*;
use crate::subsystem_manager::{GlobalDataSubsystem, SubsystemInterface};

#[test]
fn boot_and_subsystem_crc_keep_cpp_line_boundaries() {
    const CHILD: &str = "GENERALS_INI_CRC_OWNER_CHILD";
    const TEST: &str = "cnc_game_engine::ini_crc_boot::ini_crc_owner_tests::boot_and_subsystem_crc_keep_cpp_line_boundaries";
    if std::env::var_os(CHILD).is_none() {
        // Fresh process and source directory keep boot globals/assets out of this fixture.
        let dir = tempfile::tempdir().unwrap();
        let ini_dir = dir.path().join("Data/INI");
        std::fs::create_dir_all(ini_dir.join("Default")).unwrap();
        std::fs::write(
            ini_dir.join("Default/GameData.ini"),
            "GameData\nWindowed = Yes\nEnd\n",
        )
        .unwrap();
        std::fs::write(
            ini_dir.join("GameData.ini"),
            "GameData\nWindowed = No\nEnd\n",
        )
        .unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg(TEST)
            .arg("--nocapture")
            .current_dir(dir.path())
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "child stdout: {}\nchild stderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("1 passed"),
            "child filter must execute the fixture"
        );
        return;
    }
    // C++ addCRC/xferImplementation/getCRC on six separately normalized lines.
    let expected = 0xf31b_8e8au32.to_be();
    assert_eq!(
        GlobalDataSubsystem::new().calculate_xfer_crc(),
        Some(expected)
    );
    assert_eq!(calculate_game_engine_ini_crc(|_| None), expected);
}

#[test]
fn crc_accounting_does_not_readmit_native_object_modules_or_change_game_data() {
    const CHILD: &str = "GENERALS_INI_CRC_NO_READMISSION_CHILD";
    const TEST: &str = "cnc_game_engine::ini_crc_boot::ini_crc_owner_tests::crc_accounting_does_not_readmit_native_object_modules_or_change_game_data";
    if std::env::var_os(CHILD).is_none() {
        let dir = tempfile::tempdir().unwrap();
        let ini_dir = dir.path().join("Data/INI/Default");
        std::fs::create_dir_all(&ini_dir).unwrap();
        std::fs::write(ini_dir.join("Object.ini"), object_source()).unwrap();
        std::fs::write(
            ini_dir.join("GameData.ini"),
            "GameData\n Windowed = Yes\nEnd\n",
        )
        .unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg(TEST)
            .arg("--nocapture")
            .current_dir(dir.path())
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "child stdout: {}\nchild stderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        return;
    }
    use game_engine::common::thing::{
        module_factory::ModuleFactory,
        thing_factory::{ThingFactory, get_thing_factory},
    };
    let mut catalog = ModuleFactory::new();
    gamelogic::contain_module_overrides::register_native_module_overrides(&mut catalog).unwrap();
    let mut factory = ThingFactory::new();
    assert_eq!(
        factory.load_ini_text_with_module_factory(object_source(), &mut catalog),
        1
    );
    let template = factory.find_template("TechPadCRCAdmission", false).unwrap();
    assert!(template.get_draw_module_info().iter().all(|entry| {
        entry
            .data
            .as_any()
            .is::<gamelogic::object::draw::W3DModelDrawModuleData>()
    }));
    let before = template.draw_module_descriptors();
    assert_eq!(before.len(), 2);
    assert_eq!(before[0].module_tag, "ModuleTag_01");
    assert_eq!(before[1].module_tag, "ModuleTag_02");
    *get_thing_factory().unwrap() = Some(factory);
    let game_data = game_engine::common::ini::ini_game_data::ensure_global_data();
    game_data.write().windowed = false;
    assert!(GlobalDataSubsystem::new().calculate_xfer_crc().is_some());
    assert!(
        !game_data.read().windowed,
        "subsystem CRC must not reapply GameData"
    );

    let crc = calculate_game_engine_ini_crc(|_| None);
    assert_ne!(crc, 0);
    // The archive/extract route must have the same read-only semantics.
    std::fs::remove_file("Data/INI/Default/Object.ini").unwrap();
    std::fs::remove_file("Data/INI/Default/GameData.ini").unwrap();
    let extracted_crc = calculate_game_engine_ini_crc(|path| match path {
        "Data/INI/Default/Object.ini" => Some(object_source().into()),
        "Data/INI/Default/GameData.ini" => Some("GameData\n Windowed = Yes\nEnd\n".into()),
        _ => None,
    });
    assert_eq!(
        extracted_crc, crc,
        "disk and extracted bytes retain exact source order and line boundaries"
    );
    let guard = get_thing_factory().unwrap();
    let current = guard
        .as_ref()
        .unwrap()
        .find_template("TechPadCRCAdmission", false)
        .unwrap();
    assert!(current.get_draw_module_info().iter().all(|entry| {
        entry
            .data
            .as_any()
            .is::<gamelogic::object::draw::W3DModelDrawModuleData>()
    }));
    let after = current.draw_module_descriptors();
    assert_eq!(
        after.len(),
        before.len(),
        "CRC-only pass must not append duplicate Draw modules"
    );
    for (actual, expected) in after.iter().zip(&before) {
        assert_eq!(actual.name, expected.name);
        assert_eq!(actual.module_tag, expected.module_tag);
    }
    assert!(
        !game_data.read().windowed,
        "CRC-only pass must not reapply GameData"
    );
    assert!(
        std::sync::Arc::ptr_eq(&template, &current),
        "CRC accounting must not replace the admitted immutable definition"
    );
}

fn object_source() -> &'static str {
    // Retail TechReinforcementPad has two distinct authored W3DModelDraw tags.
    // This reduced source preserves that relevant grammar and nested End order.
    "Object TechPadCRCAdmission\n Draw = W3DModelDraw ModuleTag_01\n DefaultConditionState\n Model = Pad\n End\n End\n Draw = W3DModelDraw ModuleTag_02\n DefaultConditionState\n Model = Flag\n End\n End\nEnd\n"
}
