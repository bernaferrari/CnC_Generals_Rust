use game_engine::common::ini::ini::INI;
use game_engine::common::ini::ini_weapon::{get_weapon_store, reset_weapon_store};
use std::{fs, path::PathBuf};

#[test]
fn retail_windows_game_weapon_ini_parses_when_present() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../windows_game/extracted_big_files_v2/INI/Weapon.ini");
    let Ok(source) = fs::read_to_string(&path) else {
        return;
    };

    reset_weapon_store();
    let mut ini = INI::new();
    ini.with_inline_source(&source, |ini| ini.parse_current_file())
        .unwrap_or_else(|error| panic!("retail {} must parse: {error:?}", path.display()));

    let store = get_weapon_store().expect("weapon store initialized");
    assert!(store.get_template_count() >= 300);
}
