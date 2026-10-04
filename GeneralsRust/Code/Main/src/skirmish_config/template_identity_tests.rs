//! Actual authored skirmish identity across name-key namespaces.
use super::*;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn authored_skirmish_template_resolution_survives_a_foreign_key_namespace() {
    if !isolated_catalog_test(concat!(
        module_path!(),
        "::authored_skirmish_template_resolution_survives_a_foreign_key_namespace"
    )) {
        return;
    }
    use game_engine::common::ini::INI;
    use game_engine::common::name_key_generator::NameKeyGenerator;
    use game_engine::common::rts::player_template::{
        PlayerTemplateStore, get_player_template_store, get_player_template_store_mut,
    };
    // Common integration fixture contains unchanged retail blocks, not
    // synthetic starting-building records.
    const AUTHORED: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../GameEngine/Common/tests/fixtures/hq_gleoa_player_templates.ini"
    ));
    struct RestoreStore(Option<PlayerTemplateStore>);
    impl Drop for RestoreStore {
        fn drop(&mut self) {
            *get_player_template_store_mut() = self.0.take().unwrap();
        }
    }
    let _restore = RestoreStore(Some(std::mem::replace(
        &mut *get_player_template_store_mut(),
        PlayerTemplateStore::new(),
    )));
    let civilian_key = std::thread::spawn(|| {
        NameKeyGenerator::init();
        let key = NameKeyGenerator::name_to_key("FactionCivilian");
        INI::new()
            .with_inline_source(AUTHORED, |ini| ini.parse_current_file())
            .expect("actual Common authored INI parse");
        assert_eq!(get_player_template_store().len(), 6);
        key
    })
    .join()
    .unwrap();
    std::thread::spawn(move || {
        assert_eq!(
            NameKeyGenerator::name_to_key("FactionAmerica"),
            civilian_key
        );
        let selection = SkirmishPlayerTemplateSelection::base_faction("USA");
        assert_eq!(
            selection,
            SkirmishPlayerTemplateSelection::Exact {
                template_name: "FactionAmerica".to_string(),
                template_index: 2,
            },
            "explicit USA selection must retain its authored identity"
        );
        let (identity, team) = resolve_exact_skirmish_template("FactionAmerica", 2)
            .expect("real Main indexed resolution/validation");
        assert_eq!(team, Team::USA);
        assert_eq!(
            identity.resolve().unwrap().get_starting_building(),
            "AmericaCommandCenter"
        );
        assert!(
            resolve_exact_skirmish_template("FactionAmerica", 0).is_err(),
            "stale name/index pair rejected"
        );
        assert!(
            resolve_exact_skirmish_template("FactionCivilian", 0)
                .unwrap_err()
                .contains("no Skirmish StartingBuilding")
        );
        assert_eq!(
            resolve_exact_skirmish_template("FactionObserver", 1)
                .unwrap()
                .1,
            Team::Neutral
        );
        let candidates = random_skirmish_template_candidates(false);
        assert!(
            candidates
                .iter()
                .any(|(identity, _)| identity.template_name == "FactionAmerica")
        );
        assert!(
            candidates
                .iter()
                .any(|(identity, _)| identity.template_name == "FactionGLA")
        );
        assert!(
            candidates
                .iter()
                .all(|(identity, _)| identity.template_name != "FactionCivilian"
                    && identity.template_name != "FactionObserver")
        );
    })
    .join()
    .expect("real Main consumer thread");
}

// Isolate catalog mutations and bound any accidental parser lock reentry.
fn isolated_catalog_test(test_name: &str) -> bool {
    const CHILD_MARKER: &str = "GENERALS_PLAYER_TEMPLATE_IDENTITY_CHILD";
    let child_marker = CHILD_MARKER;
    let test_name = test_name
        .strip_prefix("generals_main::")
        .unwrap_or(test_name);
    if std::env::var_os(child_marker).is_some() {
        return true;
    }

    let mut child = Command::new(std::env::current_exe().expect("current test executable"))
        .args([test_name, "--exact", "--test-threads=1", "--nocapture"])
        .env(child_marker, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn regression child");
    // Drain both pipes while waiting: a long panic backtrace must not fill a
    // pipe and masquerade as a lock-reentry stall. Each reader owns its pipe.
    let readers = [
        child
            .stdout
            .take()
            .map(|pipe| std::thread::spawn(move || read_output(pipe))),
        child
            .stderr
            .take()
            .map(|pipe| std::thread::spawn(move || read_output(pipe))),
    ];
    let deadline = Instant::now() + Duration::from_secs(25);
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait().expect("regression child status") {
            break status;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            // The child may have exited between try_wait and kill. Reaping is
            // required in either case; a stalled child never escapes the test.
            let _ = child.kill();
            break child.wait().expect("reap regression child");
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let [stdout, stderr] = readers.map(|reader| {
        reader
            .expect("piped child output")
            .join()
            .expect("output reader panicked")
            .expect("read child output")
    });
    let stdout = String::from_utf8_lossy(&stdout);
    let stderr = String::from_utf8_lossy(&stderr);
    assert!(
        !timed_out,
        "{test_name} exceeded deadline (possible lock reentry): {stdout}{stderr}"
    );
    assert!(status.success(), "{test_name}: {stdout}{stderr}");
    assert!(
        stdout.contains("1 passed; 0 failed"),
        "exact regression child must run one test: {stdout}{stderr}"
    );
    false
}

fn read_output(mut pipe: impl Read) -> std::io::Result<Vec<u8>> {
    let mut output = Vec::new();
    pipe.read_to_end(&mut output)?;
    Ok(output)
}
