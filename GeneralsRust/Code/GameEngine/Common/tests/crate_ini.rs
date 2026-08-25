use game_engine::common::ini::ini::{INI, INIResult};
use game_engine::common::ini::ini_crate::{get_crate_system, initialize_crate_system};
use std::sync::Mutex;
use std::{fs, path::PathBuf};

static CRATE_TEST_LOCK: Mutex<()> = Mutex::new(());

fn parse(source: &str) -> INIResult<()> {
    let mut ini = INI::new();
    ini.with_inline_source(source, |ini| ini.parse_current_file())
}

#[test]
fn crate_data_consumes_fields_inside_the_block() {
    let _guard = CRATE_TEST_LOCK.lock().expect("crate test lock");
    initialize_crate_system();
    parse(
        r#"
CrateData DefaultCrate
  CreationChance = 0.25
  VeterancyLevel = ELITE
  KilledByType = SALVAGER INFANTRY
  KillerScience = SCIENCE_GLA
  CrateObject = 1000DollarCrate 0.75
  CrateObject = SmallLevelUpCrate 0.25
  OwnedByMaker = Yes
End
"#,
    )
    .expect("retail CrateData syntax should parse");

    let system = get_crate_system().expect("crate system initialized");
    let guard = system.read();
    let template = guard.get("DefaultCrate").expect("template stored");
    assert_eq!(template.creation_chance, 0.25);
    assert_eq!(template.veterancy_level, "ELITE");
    assert_ne!(template.killed_by_type_kindof, 0);
    assert_eq!(template.killer_science, "SCIENCE_GLA");
    assert_eq!(template.possible_crates.len(), 2);
    assert_eq!(template.possible_crates[0].crate_name, "1000DollarCrate");
    assert_eq!(template.possible_crates[0].crate_chance, 0.75);
    assert!(template.is_owned_by_maker);
}

#[test]
fn later_crate_data_inherits_default_fields() {
    let _guard = CRATE_TEST_LOCK.lock().expect("crate test lock");
    initialize_crate_system();
    parse(
        r#"
CrateData DefaultCrate
  CreationChance = 0.5
  OwnedByMaker = Yes
End

CrateData MissionCrate
  CrateObject = 200DollarCrate 1.0
End
"#,
    )
    .expect("multiple CrateData blocks should parse");

    let system = get_crate_system().expect("crate system initialized");
    let guard = system.read();
    let template = guard.get("MissionCrate").expect("derived template stored");
    assert_eq!(template.creation_chance, 0.5);
    assert!(template.is_owned_by_maker);
    assert_eq!(template.possible_crates.len(), 1);
}

#[test]
fn retail_windows_game_crate_ini_parses_when_present() {
    let _guard = CRATE_TEST_LOCK.lock().expect("crate test lock");
    initialize_crate_system();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../windows_game/extracted_big_files_v2/INIZH/Data/INI/Crate.ini");
    let Ok(source) = fs::read_to_string(&path) else {
        return;
    };

    parse(&source)
        .unwrap_or_else(|error| panic!("retail {} must parse: {error:?}", path.display()));

    let system = get_crate_system().expect("crate system initialized");
    let guard = system.read();
    assert_eq!(guard.len(), 8);
    assert!(guard.get("SalvageCrateData").is_some());
}
