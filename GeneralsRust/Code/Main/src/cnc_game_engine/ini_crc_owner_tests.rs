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

#[test]
fn captured_object_crc_uses_exact_frozen_order_without_object_reread() {
    const CHILD: &str = "GENERALS_CAPTURED_OBJECT_CRC_CHILD";
    const TEST: &str = "cnc_game_engine::ini_crc_boot::ini_crc_owner_tests::captured_object_crc_uses_exact_frozen_order_without_object_reread";
    if std::env::var_os(CHILD).is_none() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("Data/INI/Default")).unwrap();
        // A mutable disk replacement must not supersede captured Object bytes.
        std::fs::write(
            dir.path().join("Data/INI/Default/Object.ini"),
            "Object WrongDisk\nScale = 99\nEnd",
        )
        .unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .current_dir(dir.path())
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        return;
    }
    let sources = vec![
        (
            "Data/INI/Default/Object.ini".into(),
            "Object CRCOverride\nScale = 2\nEnd".into(),
        ),
        (
            "Data/INI/Object/A.ini".into(),
            "Object CRCOverride\nScale = 3\nEnd".into(),
        ),
        (
            "Data/INI/Object/Nested/Z.ini".into(),
            "Object CRCOverride\nScale = 4\nEnd".into(),
        ),
    ];
    const BEFORE: &str = "GameData\nWindowed = Yes\nEnd";
    const AFTER: &str = "Upgrade CRCUpgrade\nBuildCost = 17\nEnd";
    let crc = calculate_game_engine_ini_crc_with_object_sources(
        |path| {
            assert!(
                !path.eq_ignore_ascii_case("Data/INI/Default/Object.ini")
                    && !path.to_ascii_lowercase().starts_with("data/ini/object/"),
                "captured Object reread {path}"
            );
            match path {
                "Data/INI/Default/GameData.ini" => Some(BEFORE.into()),
                "Data/INI/Default/Upgrade.ini" => Some(AFTER.into()),
                _ => None,
            }
        },
        &sources,
    );
    // Independent ordered transfer, not the production path enumerator.
    let mut ini = INI::new();
    ini.set_xfer(XferCRC::new(XferLoad::new(Cursor::new(Vec::new()), 1)));
    for text in [
        BEFORE,
        sources[0].1.as_str(),
        sources[1].1.as_str(),
        sources[2].1.as_str(),
        AFTER,
    ] {
        ini.with_inline_source(text, INI::consume_remaining_lines)
            .unwrap();
    }
    let mut xfer = ini.take_xfer().unwrap();
    xfer.close().unwrap();
    assert_eq!(crc, xfer.get_crc());
    let mut reversed = sources.clone();
    reversed.swap(1, 2);
    let reordered = calculate_game_engine_ini_crc_with_object_sources(
        |path| match path {
            "Data/INI/Default/GameData.ini" => Some(BEFORE.into()),
            "Data/INI/Default/Upgrade.ini" => Some(AFTER.into()),
            _ => None,
        },
        &reversed,
    );
    assert_ne!(
        crc, reordered,
        "CRC must preserve ordered captured override input"
    );
}
