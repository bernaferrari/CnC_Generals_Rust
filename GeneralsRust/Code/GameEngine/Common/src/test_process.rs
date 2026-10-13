//! Keep classic Common fixture globals private to one bounded exact test process.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub(crate) enum TestProcess {
    ParentVerified,
    Child,
}

pub(crate) fn run_bounded(module: &str, name: &str) -> TestProcess {
    const CHILD_MARKER: &str = "GENERALS_COMMON_EXACT_TEST_CHILD";
    let full_name = format!("{module}::{name}");
    let test_name = full_name
        .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
        .expect("test belongs to this crate");
    if std::env::var(CHILD_MARKER).ok().as_deref() == Some(test_name) {
        return TestProcess::Child;
    }

    let mut child = Command::new(std::env::current_exe().expect("current test executable"))
        .args([
            test_name,
            "--exact",
            "--test-threads=1",
            "--nocapture",
            "--include-ignored",
        ])
        .env(CHILD_MARKER, test_name)
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
        "{test_name} exceeded deadline: {stdout}{stderr}"
    );
    assert!(status.success(), "{test_name}: {stdout}{stderr}");
    assert!(
        stdout.contains("1 passed; 0 failed"),
        "exact regression child must run one test: {stdout}{stderr}"
    );
    TestProcess::ParentVerified
}

fn read_output(mut pipe: impl Read) -> std::io::Result<Vec<u8>> {
    let mut output = Vec::new();
    pipe.read_to_end(&mut output)?;
    Ok(output)
}
