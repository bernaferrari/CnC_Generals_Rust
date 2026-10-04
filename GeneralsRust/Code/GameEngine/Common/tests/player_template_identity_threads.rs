#![cfg(not(target_arch = "wasm32"))]

//! Authored player-template identity across name-key namespaces.
//! Each test isolates the temporary global catalog in its own child process.
use game_engine::common::ini::INI;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::player_template::{
    PlayerTemplateStore, get_player_template_store, get_player_template_store_mut,
};
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const AUTHORED: &str = include_str!("fixtures/hq_gleoa_player_templates.ini");

struct RestoreStore(Option<PlayerTemplateStore>);
impl Drop for RestoreStore {
    fn drop(&mut self) {
        *get_player_template_store_mut() = self.0.take().expect("restore actual catalog");
    }
}

fn parse(source: &str) {
    INI::new()
        .with_inline_source(source, |ini| ini.parse_current_file())
        .expect("actual Common PlayerTemplate parser");
}

fn authored_catalog() -> (RestoreStore, u32) {
    let prior = std::mem::replace(
        &mut *get_player_template_store_mut(),
        PlayerTemplateStore::new(),
    );
    let restore = RestoreStore(Some(prior));
    let civilian_key = std::thread::spawn(|| {
        // Fresh loader thread owns this generator namespace. The retained
        // global store is then consumed from another fresh thread.
        NameKeyGenerator::init();
        let key = NameKeyGenerator::name_to_key("FactionCivilian");
        assert_eq!(key, 1);
        parse(AUTHORED);
        let store = get_player_template_store();
        assert_eq!(store.len(), 6);
        assert_eq!(
            store.get_nth_player_template(0).unwrap().get_name(),
            "FactionCivilian"
        );
        assert_eq!(
            store.find_template("FactionCivilian").unwrap().name_key,
            key
        );
        assert_eq!(
            store
                .find_template("FactionAmerica")
                .unwrap()
                .get_starting_building(),
            "AmericaCommandCenter"
        );
        assert_eq!(
            store
                .find_template("FactionGLA")
                .unwrap()
                .get_starting_building(),
            "GLACommandCenter"
        );
        key
    })
    .join()
    .expect("loader thread");
    (restore, civilian_key)
}

#[test]
fn authored_player_template_exact_names_survive_a_foreign_key_namespace() {
    if !isolated_catalog_test(
        "authored_player_template_exact_names_survive_a_foreign_key_namespace",
    ) {
        return;
    }
    let (_restore, civilian_key) = authored_catalog();
    std::thread::spawn(move || {
        let foreign_key = NameKeyGenerator::name_to_key("FactionAmerica");
        assert_eq!(
            foreign_key, civilian_key,
            "actual numeric collision across existing namespaces"
        );
        let store = get_player_template_store();
        for name in [
            "FactionAmerica",
            "FactionGLA",
            "FactionAmericaLaserGeneral",
            "FactionChinaTankGeneral",
        ] {
            let index = store
                .find_template_index(name)
                .expect("canonical authored name");
            let record = store.get_nth_player_template(index).unwrap();
            assert_eq!(
                record.get_name(),
                name,
                "string lookup must not alias a foreign numeric key"
            );
            assert!(!record.get_starting_building().is_empty());
        }
        assert!(
            store.find_template_index("factionamerica").is_none(),
            "exact parser/name-key case semantics"
        );
        assert!(
            store.find_template_index("FactionMissing").is_none(),
            "missing name remains absent"
        );
        assert_eq!(
            store.get_template_num_by_name("factionamerica"),
            2,
            "separate C++ no-case API unchanged"
        );
        assert!(store.get_nth_player_template_signed(-1).is_none());
        assert!(
            store
                .find_template("FactionCivilian")
                .unwrap()
                .get_starting_building()
                .is_empty()
        );
        assert!(
            store
                .find_template("FactionObserver")
                .unwrap()
                .is_observer()
        );
    })
    .join()
    .expect("consumer thread");
}

#[test]
fn authored_player_template_reparse_updates_the_same_name_from_a_foreign_thread() {
    if !isolated_catalog_test(
        "authored_player_template_reparse_updates_the_same_name_from_a_foreign_thread",
    ) {
        return;
    }
    let (_restore, civilian_key) = authored_catalog();
    std::thread::spawn(move || {
        assert_eq!(
            NameKeyGenerator::name_to_key("FactionAmerica"),
            civilian_key
        );
        // Reparse the exact original authored America block, with no invented
        // fields, neutral default, or replacement template injection.
        let start = AUTHORED.find("PlayerTemplate FactionAmerica\n").unwrap();
        let end = start + AUTHORED[start..].find("\nEnd").unwrap() + "\nEnd".len();
        parse(&AUTHORED[start..end]);
        let store = get_player_template_store();
        assert_eq!(
            store.len(),
            6,
            "reparse in place preserves store ordering/count"
        );
        assert!(
            store
                .get_nth_player_template(0)
                .unwrap()
                .get_starting_building()
                .is_empty(),
            "civilian record cannot inherit America fields"
        );
        let america = store.get_nth_player_template(2).unwrap();
        assert_eq!(america.get_name(), "FactionAmerica");
        assert_eq!(america.get_starting_building(), "AmericaCommandCenter");
        assert_eq!(america.get_side(), "America");
    })
    .join()
    .expect("reparse thread");
}

// Isolate catalog mutations and bound any accidental parser lock reentry.
fn isolated_catalog_test(test_name: &str) -> bool {
    const CHILD_MARKER: &str = "GENERALS_PLAYER_TEMPLATE_IDENTITY_CHILD";
    let child_marker = CHILD_MARKER;
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
