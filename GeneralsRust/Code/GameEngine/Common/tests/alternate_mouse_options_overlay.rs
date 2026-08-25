//! Startup parity for `GlobalData::parseGameDataDefinition`.
//!
//! C++ loads `Options.ini` after GameData and lets `UseAlternateMouse` override
//! the authored GameData value. Keep this as an integration test because Common
//! intentionally disables its library test harness in `Cargo.toml`.

use game_engine::common::global_data as runtime_global_data;
use game_engine::common::ini::ini::{INI, INIResult};
use game_engine::common::ini::ini_game_data::{GlobalData, ensure_global_data};
use std::sync::Mutex;

static GLOBAL_DATA_TEST_LOCK: Mutex<()> = Mutex::new(());

fn parse(source: &str) -> INIResult<()> {
    let mut ini = INI::new();
    ini.with_inline_source(source, |ini| ini.parse_current_file())
}

#[test]
fn startup_game_data_load_overlays_options_ini_alternate_mouse_to_both_global_residences() {
    let _guard = GLOBAL_DATA_TEST_LOCK.lock().expect("global data test lock");
    let ini_global = ensure_global_data();
    let previous_ini = ini_global.read().clone();
    let previous_runtime = runtime_global_data::read().clone();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let user_data = tempfile::tempdir().expect("temporary user-data directory");
        std::fs::write(
            user_data.path().join("Options.ini"),
            "UseAlternateMouse = YES\n",
        )
        .expect("temporary Options.ini");

        {
            let mut data = ini_global.write();
            *data = GlobalData::new();
            data.init();
            data.set_path_user_data(user_data.path().to_string_lossy().into_owned());
            data.use_alternate_mouse = false;
        }
        *runtime_global_data::write() = runtime_global_data::GlobalData::default();

        parse("GameData\nEnd\n").expect("minimal GameData block parses");

        assert!(
            ini_global.read().use_alternate_mouse,
            "Options.ini must override the authored GameData value"
        );
        assert!(
            runtime_global_data::read().use_alternate_mouse,
            "the startup overlay must reach Main's runtime GlobalData residence"
        );
    }));

    *ini_global.write() = previous_ini;
    *runtime_global_data::write() = previous_runtime;
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

#[test]
fn startup_game_data_load_overlays_scroll_retaliation_double_click_gamma() {
    let _guard = GLOBAL_DATA_TEST_LOCK.lock().expect("global data test lock");
    let ini_global = ensure_global_data();
    let previous_ini = ini_global.read().clone();
    let previous_runtime = runtime_global_data::read().clone();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let user_data = tempfile::tempdir().expect("temporary user-data directory");
        std::fs::write(
            user_data.path().join("Options.ini"),
            "ScrollFactor = 25\nRetaliation = no\nUseDoubleClickAttackMove = yes\nGamma = 100\n",
        )
        .expect("temporary Options.ini");

        {
            let mut data = ini_global.write();
            *data = GlobalData::new();
            data.init();
            data.set_path_user_data(user_data.path().to_string_lossy().into_owned());
            data.keyboard_scroll_factor = 2.0;
            data.keyboard_default_scroll_factor = 0.5;
            data.client_retaliation_mode_enabled = true;
            data.double_click_attack_move = false;
            data.display_gamma = 1.0;
        }
        *runtime_global_data::write() = runtime_global_data::GlobalData::default();

        parse("GameData\n  KeyboardScrollSpeedFactor = 2.0\nEnd\n")
            .expect("GameData with retail scroll factor parses");

        let ini = ini_global.read();
        assert!(
            (ini.keyboard_scroll_factor - 0.25).abs() < 0.001,
            "ScrollFactor must overlay GameData KeyboardScrollSpeedFactor, got {}",
            ini.keyboard_scroll_factor
        );
        assert!(!ini.client_retaliation_mode_enabled);
        assert!(ini.double_click_attack_move);
        assert!((ini.display_gamma - 2.0).abs() < 0.01);

        let runtime = runtime_global_data::read();
        assert!((runtime.keyboard_scroll_factor - 0.25).abs() < 0.001);
        assert!(!runtime.client_retaliation_mode_enabled);
        assert!(runtime.double_click_attack_move);
        assert!((runtime.display_gamma - 2.0).abs() < 0.01);
    }));

    *ini_global.write() = previous_ini;
    *runtime_global_data::write() = previous_runtime;
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

#[test]
fn startup_missing_scroll_factor_uses_keyboard_default_not_gamedata() {
    let _guard = GLOBAL_DATA_TEST_LOCK.lock().expect("global data test lock");
    let ini_global = ensure_global_data();
    let previous_ini = ini_global.read().clone();
    let previous_runtime = runtime_global_data::read().clone();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let user_data = tempfile::tempdir().expect("temporary user-data directory");
        std::fs::write(
            user_data.path().join("Options.ini"),
            "UseAlternateMouse = no\n",
        )
        .expect("temporary Options.ini");

        {
            let mut data = ini_global.write();
            *data = GlobalData::new();
            data.init();
            data.set_path_user_data(user_data.path().to_string_lossy().into_owned());
            data.keyboard_default_scroll_factor = 0.5;
        }
        *runtime_global_data::write() = runtime_global_data::GlobalData::default();

        parse("GameData\n  KeyboardScrollSpeedFactor = 2.0\nEnd\n")
            .expect("GameData with retail scroll factor parses");

        assert!(
            (ini_global.read().keyboard_scroll_factor - 0.5).abs() < 0.001,
            "missing ScrollFactor must use keyboardDefaultScrollFactor"
        );
        assert!((runtime_global_data::read().keyboard_scroll_factor - 0.5).abs() < 0.001);
    }));

    *ini_global.write() = previous_ini;
    *runtime_global_data::write() = previous_runtime;
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}
