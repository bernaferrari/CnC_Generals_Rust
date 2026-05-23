use game_engine::common::ini::INI;
use game_engine::common::language::Language;
use game_engine::common::rts::player_template::{
    get_player_template_store, get_player_template_store_mut,
};
use std::sync::{Mutex, OnceLock};

static PLAYER_TEMPLATE_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    PLAYER_TEMPLATE_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[test]
fn player_template_display_name_translates_full_labels_and_ini_labels() {
    let _guard = test_lock();
    Language::clear_localized_strings();
    Language::register_localized_string("GUI:TestFaction", "Translated GUI Faction");
    Language::register_localized_string("TestIniFaction", "Translated INI Faction");
    get_player_template_store_mut().clear();

    let source = r#"
PlayerTemplate TestGuiFaction
  DisplayName = GUI:TestFaction
End

PlayerTemplate TestIniFaction
  DisplayName = INI:TestIniFaction
End
"#;

    let mut ini = INI::new();
    ini.with_inline_source(source, |ini| ini.parse_current_file())
        .expect("inline PlayerTemplate parse");

    let store = get_player_template_store();
    assert_eq!(
        store
            .find_template("TestGuiFaction")
            .expect("GUI template")
            .get_display_name(),
        "Translated GUI Faction"
    );
    assert_eq!(
        store
            .find_template("TestIniFaction")
            .expect("INI template")
            .get_display_name(),
        "Translated INI Faction"
    );

    drop(store);
    get_player_template_store_mut().clear();
    Language::clear_localized_strings();
}
