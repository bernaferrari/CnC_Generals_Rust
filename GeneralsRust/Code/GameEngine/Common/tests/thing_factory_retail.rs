use game_engine::common::thing::thing_factory::ThingFactory;
use std::{fs, path::PathBuf};

struct TestLogger;
impl log::Log for TestLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Warn
    }
    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            eprintln!("{}", record.args());
        }
    }
    fn flush(&self) {}
}
static LOGGER: TestLogger = TestLogger;

#[test]
fn retail_object_ini_set_populates_gameplay_templates() {
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(log::LevelFilter::Warn);
    let object_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../windows_game/extracted_big_files_v2/INI/Object");
    let Ok(entries) = fs::read_dir(&object_dir) else {
        return;
    };
    let mut paths: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("ini"))
        })
        .collect();
    paths.sort();

    let mut factory = ThingFactory::new();
    let mut loaded = 0usize;
    for path in paths {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        loaded += factory.load_ini_text(&source);
    }
    assert!(
        loaded >= 2_100,
        "retail gameplay template coverage is still too low: {loaded}"
    );
    for name in [
        "AmericaVehicleDozer",
        "ChinaVehicleDozer",
        "SupW_AmericaCommandCenter",
        "Tank_ChinaTankBattleMaster",
    ] {
        assert!(
            factory.find_template(name, false).is_some(),
            "missing {name}"
        );
    }

    let gla_source = fs::read_to_string(object_dir.join("GLAInfantry.ini"))
        .expect("read retail GLAInfantry.ini");
    let mut gla_factory = ThingFactory::new();
    let gla_loaded = gla_factory.load_ini_text(&gla_source);
    let gla_declarations: Vec<_> = gla_source
        .lines()
        .filter_map(|line| {
            let mut tokens = line.split(';').next()?.split_whitespace();
            if !tokens
                .next()
                .is_some_and(|token| token.eq_ignore_ascii_case("Object"))
            {
                return None;
            }
            let name = tokens.next()?;
            (name != "=").then_some(name)
        })
        .collect();
    let missing: Vec<_> = gla_declarations
        .iter()
        .copied()
        .filter(|name| gla_factory.find_template(name, false).is_none())
        .collect();
    let present: Vec<_> = gla_declarations
        .iter()
        .copied()
        .filter(|name| gla_factory.find_template(name, false).is_some())
        .collect();
    assert!(
        gla_factory
            .find_template("GLAInfantryWorker", false)
            .is_some(),
        "GLAInfantryWorker missing after loading {gla_loaded} GLA infantry templates; present {present:?}; missing {missing:?}"
    );
}
