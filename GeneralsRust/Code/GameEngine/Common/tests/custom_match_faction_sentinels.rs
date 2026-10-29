use game_engine::common::multiplayer_settings::{
    PLAYERTEMPLATE_MIN, PLAYERTEMPLATE_OBSERVER, PLAYERTEMPLATE_RANDOM,
};
use game_engine::common::preferences::CustomMatchPreferences;
use game_engine::common::rts::player_template::{PlayerTemplate, get_player_template_store_mut};

#[test]
fn offline_preferred_faction_uses_cpp_sentinels_and_preserves_faction_one() {
    let mut store = get_player_template_store_mut();
    store.clear();
    for name in ["FactionIndexZero", "FactionIndexOne", "FactionIndexTwo"] {
        let mut template = PlayerTemplate::new(name.to_owned());
        template.starting_building = "CommandCenter".into();
        store.add_template(template);
    }
    drop(store);

    let mut prefs = CustomMatchPreferences::new();
    prefs.set_preferred_faction(1);
    assert_eq!(
        prefs.get_preferred_faction(),
        1,
        "real faction index 1 is not OBSERVER"
    );

    prefs.set_preferred_faction(PLAYERTEMPLATE_RANDOM);
    assert_eq!(prefs.get_preferred_faction(), PLAYERTEMPLATE_RANDOM);
    prefs.set_preferred_faction(PLAYERTEMPLATE_OBSERVER);
    assert_eq!(prefs.get_preferred_faction(), PLAYERTEMPLATE_RANDOM);
    prefs.set_preferred_faction(PLAYERTEMPLATE_MIN - 1);
    assert_eq!(prefs.get_preferred_faction(), PLAYERTEMPLATE_RANDOM);
    prefs.set_preferred_faction(3);
    assert_eq!(prefs.get_preferred_faction(), PLAYERTEMPLATE_RANDOM);

    get_player_template_store_mut().clear();
}
