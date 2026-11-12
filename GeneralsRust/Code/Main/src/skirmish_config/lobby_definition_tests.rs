//! Minimal immutable definition inputs for process-isolated slot witnesses.
//! These exercise admission identities, not retail asset loading. The containing
//! skirmish tests retain retail content, General templates and science coverage.

/// Original PlayerTemplate fields identify the base sides. StartingBuilding
/// satisfies the real skirmish admission precondition.
const SLOT_PLAYER_TEMPLATES: &str = "PlayerTemplate FactionAmerica\n Side = America\n BaseSide = America\n StartingBuilding = AmericaCommandCenter\nEnd\n\
     PlayerTemplate FactionGLA\n Side = GLA\n BaseSide = GLA\n StartingBuilding = GLACommandCenter\nEnd\n\
     PlayerTemplate FactionObserver\n IsObserver = Yes\n Side = Observer\n BaseSide = Observer\nEnd\n\
     PlayerTemplate FactionCivilian\n Side = Civilian\n BaseSide = Civilian\nEnd\n";

/// The slot witnesses' PlayerTemplate catalog, parsed into a store owned by
/// the caller (C++ GameEngine.cpp:428 loads it before any NewGame). Skirmish
/// admission reads the world's own catalog, not the process-wide store.
pub(crate) fn slot_definitions() -> game_engine::common::rts::player_template::PlayerTemplateStore {
    let mut definitions = game_engine::common::rts::player_template::PlayerTemplateStore::new_owned();
    definitions
        .apply_source(SLOT_PLAYER_TEMPLATES, game_engine::common::ini::INILoadType::Overwrite)
        .expect("admit slot identity definition inputs");
    definitions
}

/// A fresh world that owns the slot PlayerTemplate catalog and the ordered
/// Object content its StartingBuildings name, as the application boot does.
pub(crate) fn slot_world() -> crate::game_logic::GameLogic {
    slot_world_with("")
}

/// A slot world whose ordered content also authors `objects`.
pub(crate) fn slot_world_with_objects(
    objects: &[crate::fixture_content::FixtureObject],
) -> crate::game_logic::GameLogic {
    let mut world = crate::game_logic::GameLogic::new();
    admit_slot_world_with_objects(&mut world, &slot_definitions(), objects);
    world
}

/// A slot world whose own PlayerTemplate catalog also applies `overrides`
/// (later INI blocks overwrite earlier ones, as C++ INI_LOAD_OVERWRITE).
/// Fixtures author template fields here, never in the process-wide store.
pub(crate) fn slot_world_with(overrides: &str) -> crate::game_logic::GameLogic {
    let mut definitions = slot_definitions();
    if !overrides.is_empty() {
        definitions
            .apply_source(overrides, game_engine::common::ini::INILoadType::Overwrite)
            .expect("admit slot PlayerTemplate overrides");
    }
    let mut world = crate::game_logic::GameLogic::new();
    admit_slot_world(&mut world, &definitions);
    world
}

pub(crate) fn admit_slot_definitions() {
    use game_engine::common::ini::INI;
    use game_engine::common::rts::player_template::get_player_template_store;

    // A second world uses the same definitions; it must not republish them.
    let present = {
        let definitions = get_player_template_store();
        [
            "FactionAmerica",
            "FactionGLA",
            "FactionObserver",
            "FactionCivilian",
        ]
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
        SLOT_PLAYER_TEMPLATES,
        |ini| ini.parse_current_file(),
    )
    .expect("admit slot identity definition inputs");
}

/// Select each exact slot's PlayerTemplate by its index in the receiving
/// world's own catalog, as the C++ lobby does against the TheThingFactory /
/// PlayerTemplateStore of the session that starts the match. The golden config
/// builder resolves indices in whatever process-wide store is ambient, which is
/// not the catalog `slot_world` admits.
pub(crate) fn slot_config(
    world: &crate::game_logic::GameLogic,
    config: &crate::skirmish_config::SkirmishMatchConfig,
) -> crate::skirmish_config::SkirmishMatchConfig {
    let definitions = world
        .player_template_definitions
        .as_ref()
        .expect("slot world owns its PlayerTemplate catalog");
    let mut config = config.clone();
    for slot in &mut config.slots {
        if let crate::skirmish_config::SkirmishPlayerTemplateSelection::Exact {
            template_name,
            template_index,
        } = &mut slot.player_template
        {
            *template_index = definitions
                .find_template_index(template_name)
                .and_then(|index| i32::try_from(index).ok())
                .unwrap_or(-1);
        }
    }
    config
}

/// Start a skirmish fixture the way the application does: the receiving world
/// owns the slot PlayerTemplate catalog (GameEngine.cpp:428 loads
/// PlayerTemplate.ini before any NewGame) and every slot selects its template
/// by index in that catalog (GameLogic.cpp:1318-1319 resolves the GameSlot's
/// PlayerTemplate index with ThePlayerTemplateStore->getNthPlayerTemplate). Fixtures
/// that author their own catalog call `slot_config` + `apply_skirmish_config`.
pub(crate) fn start_slot_skirmish(
    world: &mut crate::game_logic::GameLogic,
    config: &crate::skirmish_config::SkirmishMatchConfig,
) -> Result<(), String> {
    world.admit_player_template_definitions(slot_definitions())?;
    let config = slot_config(world, config);
    crate::skirmish_config::apply_skirmish_config(world, &config)
}

/// Give each receiving test world the same ordered authored startup content.
pub(crate) fn admit_slot_world(
    world: &mut crate::game_logic::GameLogic,
    definitions: &game_engine::common::rts::player_template::PlayerTemplateStore,
) {
    admit_slot_world_with_objects(world, definitions, &[]);
}

/// The slot catalog plus its StartingBuildings and any other Objects the
/// fixture creates or its map places, admitted as one ordered content.
pub(crate) fn admit_slot_world_with_objects(
    world: &mut crate::game_logic::GameLogic,
    definitions: &game_engine::common::rts::player_template::PlayerTemplateStore,
    objects: &[crate::fixture_content::FixtureObject],
) {
    use crate::fixture_content::FixtureObject;
    world.admit_player_template_definitions(definitions.clone()).unwrap();
    let mut content = vec![
        FixtureObject::new("AmericaCommandCenter").kind_of("STRUCTURE COMMANDCENTER").health(5000.0),
        FixtureObject::new("GLACommandCenter").kind_of("STRUCTURE COMMANDCENTER").health(5000.0),
    ];
    content.extend_from_slice(objects);
    crate::fixture_content::admit_fixture_objects(world, &content);
}

/// A Multiplayer.ini palette of `count` authored colors for a slot world.
/// C++ MultiplayerSettings always holds the authored palette before any
/// skirmish admission (GameEngine.cpp loads Multiplayer.ini at init).
pub(crate) fn slot_palette(
    count: u32,
) -> game_engine::common::ini::ini_multiplayer::MultiplayerSettings {
    let mut settings = game_engine::common::ini::ini_multiplayer::MultiplayerSettings::new();
    for index in 0..count {
        let color = settings
            .new_multiplayer_color_definition(format!("SlotColor{index}").as_str().into());
        color.set_color(0x10_0000 * (index + 1) + index);
        color.set_night_color(0x08_0000 * (index + 1) + index);
    }
    settings
}
