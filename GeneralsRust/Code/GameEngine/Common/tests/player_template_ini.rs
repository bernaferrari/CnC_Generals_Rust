use game_engine::common::ini::ini::INI;
use game_engine::common::language::Language;
use game_engine::common::rts::player_template::{
    get_player_template_store, get_player_template_store_mut,
};

fn reset_player_template_test_state() {
    Language::clear_localized_strings();
    get_player_template_store_mut().clear();
}

#[test]
fn player_template_display_name_translates_full_label_token() {
    reset_player_template_test_state();
    Language::register_localized_string("GUI:America", "United States");
    Language::register_localized_string("INI:FactionChina", "China");

    let mut ini = INI::new();
    ini.with_inline_source(
        r#"
PlayerTemplate FactionAmerica
  DisplayName = GUI:America
End

PlayerTemplate FactionChina
  DisplayName = INI:FactionChina
End
"#,
        |ini| {
            ini.parse_current_file()?;
            Ok(())
        },
    )
    .expect("inline PlayerTemplate should parse");

    {
        let store = get_player_template_store();
        let template = store
            .find_template("FactionAmerica")
            .expect("template should be stored");
        assert_eq!(template.display_name, "United States");
        let template = store
            .find_template("FactionChina")
            .expect("second template should be stored");
        assert_eq!(template.display_name, "China");
    }

    reset_player_template_test_state();
}
