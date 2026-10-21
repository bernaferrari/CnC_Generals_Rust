//! Focused developer regression for GameWorld shadow scheduling during replay fast-forward.
//!
//! Run with:
//! `cargo run -p generals_main --features internal --bin replay_fast_forward_probe`
//!
//! The parent owns child-process configuration. The child starts Tokio only
//! after its inherited environment has been fixed by Command, so no running
//! process changes the environment observed by native/runtime worker threads.

#[cfg(feature = "internal")]
const CHILD: &str = "GENERALS_REPLAY_PROBE_CHILD";
#[cfg(feature = "internal")]
const PRIVATE_ROOT: &str = "GENERALS_REPLAY_PROBE_PRIVATE_ROOT";
#[cfg(feature = "internal")]
const SOURCE_ROOT: &str = "GENERALS_REPLAY_PROBE_SOURCE_ROOT";

#[cfg(feature = "internal")]
fn probe_child_requested() -> bool {
    std::env::var_os(CHILD).as_deref() == Some(std::ffi::OsStr::new("1"))
}

#[cfg(feature = "internal")]
fn replay_probe_child_command(
    executable: &std::path::Path,
) -> anyhow::Result<std::process::Command> {
    let original = std::env::current_dir()?.canonicalize()?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    // A wall clock is not a uniqueness guarantee across parallel launchers.
    let root = create_probe_directory(nonce)?;
    let root = root.canonicalize()?;
    for directory in ["cwd", "userdata"] {
        std::fs::create_dir(root.join(directory))?;
    }
    eprintln!("replay probe private evidence: {}", root.display());
    let mut command = std::process::Command::new(executable);
    command
        .current_dir(root.join("cwd"))
        .env(PRIVATE_ROOT, &root)
        .env(SOURCE_ROOT, &original);
    // Preserve explicit licensed asset configuration and every other inherited input.
    if std::env::var_os("GENERALS_ASSETS_DIR").is_none() {
        command.env("GENERALS_ASSETS_DIR", original);
    }
    command
        .env(CHILD, "1")
        .env("GENERALS_GAMEWORLD_SHADOW", "1");
    Ok(command)
}

#[cfg(feature = "internal")]
fn create_probe_directory(nonce: u128) -> std::io::Result<std::path::PathBuf> {
    for attempt in 0..1024 {
        let root = std::env::temp_dir().join(format!(
            "generals-replay-probe-{}-{nonce}-{attempt}",
            std::process::id()
        ));
        // Exclusive creation never adopts an existing directory or symlink.
        match std::fs::create_dir(&root) {
            Ok(()) => return Ok(root),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "probe could not reserve a private directory",
    ))
}

#[cfg(feature = "internal")]
fn main() -> anyhow::Result<()> {
    if probe_child_requested() {
        let root = std::path::PathBuf::from(
            std::env::var_os(PRIVATE_ROOT)
                .ok_or_else(|| anyhow::anyhow!("probe child requires private launch root"))?,
        );
        let source = std::path::PathBuf::from(
            std::env::var_os(SOURCE_ROOT)
                .ok_or_else(|| anyhow::anyhow!("probe child requires source asset root"))?,
        );
        // Synchronous admission precedes #[tokio::main], winit and every Engine.
        generals_main::cnc_game_engine::prepare_replay_probe_private_paths(&root, &source)?;
        let result = run_probe_child();
        eprintln!("replay probe retained private output: {}", root.display());
        return result;
    }
    // Keep CLI and inherited stdio unchanged. Command owns both overrides;
    // configuring it cannot mutate this parent's environment.
    let executable = std::env::current_exe()?;
    let mut command = replay_probe_child_command(&executable)?;
    let evidence = command
        .get_envs()
        .find(|(key, _)| *key == PRIVATE_ROOT)
        .and_then(|(_, value)| value)
        .map(std::path::PathBuf::from)
        .unwrap();
    let protected = protected_outputs(&executable)?;
    let before = output_metadata(&protected)?;
    std::fs::write(
        evidence.join("protected-before.txt"),
        format!("{before:#?}"),
    )?;
    let status = command.args(std::env::args_os().skip(1)).status()?;
    let after = output_metadata(&protected)?;
    std::fs::write(evidence.join("protected-after.txt"), format!("{after:#?}"))?;
    anyhow::ensure!(
        before == after,
        "protected user/install output changed; see {}",
        evidence.display()
    );
    let actual = output_metadata(&[evidence.clone()])?;
    std::fs::write(
        evidence.join("private-output-metadata.txt"),
        format!("{actual:#?}"),
    )?;
    if !status.success() {
        // Preserve numeric child failure/panic codes without printing a second
        // copy of the probe error. Signal-only termination has no portable code.
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

#[cfg(feature = "internal")]
#[tokio::main]
async fn run_probe_child() -> anyhow::Result<()> {
    generals_main::cnc_game_engine::run_replay_fast_forward_engine_probe()
}

#[cfg(not(feature = "internal"))]
fn main() {
    eprintln!("replay_fast_forward_probe requires `--features internal`");
    std::process::exit(2);
}

#[cfg(all(test, feature = "internal", not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::io::Read;
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    const TEST: &str = "GENERALS_REPLAY_LAUNCH_TEST";
    const CASE: &str = "GENERALS_REPLAY_LAUNCH_CASE";

    fn isolated_case(
        test: &str,
        scenario: &str,
        configure: impl FnOnce(&mut std::process::Command),
        run: impl FnOnce(),
    ) {
        if std::env::var(TEST).ok().as_deref() == Some(test) {
            if std::env::var(CASE).ok().as_deref() == Some(scenario) {
                run();
            }
            return;
        }
        let mut command = replay_probe_child_command(&std::env::current_exe().unwrap()).unwrap();
        command
            .args(["--exact", test, "--nocapture", "--test-threads=1"])
            .env(TEST, test)
            .env(CASE, scenario)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        configure(&mut command);
        let mut child = command.spawn().unwrap();
        let pipes: [Box<dyn Read + Send>; 2] = [
            Box::new(child.stdout.take().unwrap()),
            Box::new(child.stderr.take().unwrap()),
        ];
        let readers = pipes.map(|mut pipe| {
            std::thread::spawn(move || {
                let mut output = Vec::new();
                pipe.read_to_end(&mut output).unwrap();
                output
            })
        });
        let deadline = Instant::now() + Duration::from_secs(30);
        let (status, timed_out) = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break (status, false);
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                break (child.wait().unwrap(), true);
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        let output =
            readers.map(|reader| String::from_utf8_lossy(&reader.join().unwrap()).into_owned());
        assert!(!timed_out, "replay launch child timed out: {output:?}");
        assert!(
            status.success(),
            "replay launch child failed: {status}: {output:?}"
        );
        assert!(
            output[0].contains("running 1 test") && output[0].contains("1 passed; 0 failed"),
            "exact replay launch child must run one passing test: {output:?}"
        );
    }

    #[test]
    fn repeated_clock_value_reserves_distinct_exclusive_directories() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let first = create_probe_directory(nonce).unwrap();
        let marker = first.join("owned-marker");
        std::fs::write(&marker, b"first launch").unwrap();
        let second = create_probe_directory(nonce).unwrap();
        assert_ne!(first, second);
        assert_eq!(std::fs::read(&marker).unwrap(), b"first launch");
        assert!(!second.join("owned-marker").exists());
        std::fs::remove_file(marker).unwrap();
        std::fs::remove_dir(first).unwrap();
        std::fs::remove_dir(second).unwrap();
    }

    #[test]
    fn launcher_installs_owned_child_environment_without_parent_mutation() {
        let before_shadow = std::env::var_os("GENERALS_GAMEWORLD_SHADOW");
        let before_child = std::env::var_os(CHILD);
        let before_home = std::env::var_os("HOME");
        let before_appdata = std::env::var_os("APPDATA");
        let before_cwd = std::env::current_dir().unwrap();
        isolated_case(
            "tests::launcher_installs_owned_child_environment_without_parent_mutation",
            "launch",
            |_| {},
            || {
                assert!(
                    probe_child_requested(),
                    "child must select runtime, not launch another child"
                );
                assert_eq!(
                    std::env::var_os("GENERALS_GAMEWORLD_SHADOW"),
                    Some("1".into())
                );
                assert!(generals_main::gameworld_shadow::gameworld_shadow_enabled());
                let root = std::path::PathBuf::from(std::env::var_os(PRIVATE_ROOT).unwrap());
                assert_eq!(std::env::current_dir().unwrap(), root.join("cwd"));
                assert!(!root.join("home").exists());
                assert!(!root.join("appdata").exists());
                assert!(!root.join("userdata/Options.ini").exists());
            },
        );
        assert_eq!(std::env::var_os("GENERALS_GAMEWORLD_SHADOW"), before_shadow);
        assert_eq!(std::env::var_os(CHILD), before_child);
        assert_eq!(std::env::var_os("HOME"), before_home);
        assert_eq!(std::env::var_os("APPDATA"), before_appdata);
        assert_eq!(std::env::current_dir().unwrap(), before_cwd);
    }

    #[test]
    fn launcher_preserves_cli_and_rejects_incorrect_child_marker() {
        let mut command = replay_probe_child_command(std::path::Path::new("replay-probe")).unwrap();
        command.args(["original-arg", "second-arg"]);
        assert!(command.get_envs().all(|(key, _)| {
            !["HOME", "USERPROFILE", "APPDATA", "LOCALAPPDATA"]
                .iter()
                .any(|name| key == std::ffi::OsStr::new(name))
        }));
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [
                std::ffi::OsStr::new("original-arg"),
                std::ffi::OsStr::new("second-arg")
            ]
        );
        isolated_case(
            "tests::launcher_preserves_cli_and_rejects_incorrect_child_marker",
            "wrong-marker",
            |command| {
                command.env(CHILD, "0");
            },
            || {
                assert!(!probe_child_requested());
            },
        );
        isolated_case(
            "tests::launcher_preserves_cli_and_rejects_incorrect_child_marker",
            "missing-marker",
            |command| {
                command.env_remove(CHILD);
            },
            || {
                assert!(!probe_child_requested());
            },
        );
    }

    #[test]
    fn probe_rejects_disabled_shadow_before_engine_creation() {
        for disabled in ["0", "false"] {
            isolated_case(
                "tests::probe_rejects_disabled_shadow_before_engine_creation",
                disabled,
                |command| {
                    command.env("GENERALS_GAMEWORLD_SHADOW", disabled);
                },
                || {
                    assert!(!generals_main::gameworld_shadow::gameworld_shadow_enabled());
                    let error = generals_main::cnc_game_engine::run_replay_fast_forward_engine_probe()
                        .expect_err("disabled inherited policy must reject before touching winit/engine");
                    assert!(
                        error
                            .to_string()
                            .contains("requires shadow enabled at process launch")
                    );
                    assert_eq!(
                        std::env::var("GENERALS_GAMEWORLD_SHADOW").unwrap(),
                        disabled
                    );
                },
            );
        }
    }

    #[test]
    fn missing_shadow_override_preserves_existing_enabled_default() {
        isolated_case(
            "tests::missing_shadow_override_preserves_existing_enabled_default",
            "missing",
            |command| {
                command.env_remove("GENERALS_GAMEWORLD_SHADOW");
            },
            || {
                assert!(std::env::var_os("GENERALS_GAMEWORLD_SHADOW").is_none());
                assert!(generals_main::gameworld_shadow::gameworld_shadow_enabled());
            },
        );
    }
}

#[cfg(feature = "internal")]
fn protected_outputs(executable: &std::path::Path) -> anyhow::Result<Vec<std::path::PathBuf>> {
    let mut paths = vec![std::env::current_dir()?.join("UserData")];
    if let Some(home) = std::env::var_os("HOME") {
        paths.push(std::path::PathBuf::from(home).join(".cnc_generals"));
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        paths.push(std::path::PathBuf::from(appdata).join("Command & Conquer Generals Zero Hour"));
    }
    if let Some(parent) = executable.parent() {
        paths.push(parent.join("Replays"));
    }
    Ok(paths)
}

#[cfg(feature = "internal")]
fn output_metadata(
    roots: &[std::path::PathBuf],
) -> anyhow::Result<
    std::collections::BTreeMap<std::path::PathBuf, (u64, Option<std::time::SystemTime>, bool)>,
> {
    fn visit(
        path: &std::path::Path,
        entries: &mut std::collections::BTreeMap<
            std::path::PathBuf,
            (u64, Option<std::time::SystemTime>, bool),
        >,
    ) -> anyhow::Result<()> {
        let metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        entries.insert(
            path.to_owned(),
            (
                metadata.len(),
                metadata.modified().ok(),
                metadata.file_type().is_symlink(),
            ),
        );
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            for entry in std::fs::read_dir(path)? {
                visit(&entry?.path(), entries)?;
            }
        }
        Ok(())
    }
    let mut entries = std::collections::BTreeMap::new();
    for root in roots {
        visit(root, &mut entries)?;
    }
    Ok(entries)
}
