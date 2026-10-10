//! Minimal immutable definition inputs for process-isolated slot witnesses.
//! These exercise admission identities, not retail asset loading. The containing
//! skirmish tests retain retail content, General templates and science coverage.

pub(crate) fn admit_slot_definitions() {
    use game_engine::common::ini::INI;
    use game_engine::common::rts::player_template::get_player_template_store;

    // A second world uses the same definitions; it must not republish them.
    let present = {
        let definitions = get_player_template_store();
        ["FactionAmerica", "FactionGLA", "FactionObserver"]
            .iter()
            .all(|name| definitions.find_template(name).is_some())
    };
    if present {
        return;
    }
    // Original PlayerTemplate fields identify the base sides. StartingBuilding satisfies
    // the real skirmish admission precondition. Other constructor defaults remain: this witness observes slot names,
    // start positions, relationships and observer admission only.
    let mut ini = INI::new();
    ini.with_inline_source(
        "PlayerTemplate FactionAmerica\n Side = America\n BaseSide = America\n StartingBuilding = AmericaCommandCenter\nEnd\n\
         PlayerTemplate FactionGLA\n Side = GLA\n BaseSide = GLA\n StartingBuilding = GLACommandCenter\nEnd\n\
         PlayerTemplate FactionObserver\n Side = Observer\n BaseSide = Observer\nEnd\n",
        |ini| ini.parse_current_file(),
    )
    .expect("admit slot identity definition inputs");
}
