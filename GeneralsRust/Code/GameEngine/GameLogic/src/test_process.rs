//! Run lock-reentry regressions in an isolated process with a finite deadline.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub(crate) enum TestProcess {
    ParentVerified,
    Child,
}

pub(crate) fn run_bounded(test_name: &str, child_marker: &str) -> TestProcess {
    if std::env::var_os(child_marker).is_some() {
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
    TestProcess::ParentVerified
}

fn read_output(mut pipe: impl Read) -> std::io::Result<Vec<u8>> {
    let mut output = Vec::new();
    pipe.read_to_end(&mut output)?;
    Ok(output)
}
