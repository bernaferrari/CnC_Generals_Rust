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
fn probe_child_requested() -> bool {
    std::env::var_os(CHILD).as_deref() == Some(std::ffi::OsStr::new("1"))
}

#[cfg(feature = "internal")]
fn replay_probe_child_command(executable: &std::path::Path) -> std::process::Command {
    let mut command = std::process::Command::new(executable);
    command
        .env(CHILD, "1")
        .env("GENERALS_GAMEWORLD_SHADOW", "1");
    command
}

#[cfg(feature = "internal")]
fn main() -> anyhow::Result<()> {
    if probe_child_requested() {
        return run_probe_child();
    }
    // Keep CLI and inherited stdio unchanged. Command owns both overrides;
    // configuring it cannot mutate this parent's environment.
    let status = replay_probe_child_command(&std::env::current_exe()?)
        .args(std::env::args_os().skip(1))
        .status()?;
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
        let mut command = replay_probe_child_command(&std::env::current_exe().unwrap());
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
    fn launcher_installs_owned_child_environment_without_parent_mutation() {
        let before_shadow = std::env::var_os("GENERALS_GAMEWORLD_SHADOW");
        let before_child = std::env::var_os(CHILD);
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
            },
        );
        assert_eq!(std::env::var_os("GENERALS_GAMEWORLD_SHADOW"), before_shadow);
        assert_eq!(std::env::var_os(CHILD), before_child);
    }

    #[test]
    fn launcher_preserves_cli_and_rejects_incorrect_child_marker() {
        let mut command = replay_probe_child_command(std::path::Path::new("replay-probe"));
        command.args(["original-arg", "second-arg"]);
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
