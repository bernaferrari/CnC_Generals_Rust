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
