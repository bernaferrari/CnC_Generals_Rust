"""Source contracts for linked non-network CI prerequisites and required gates."""

import re
import shlex
import unittest
from pathlib import Path


WORKFLOW = Path(__file__).resolve().parents[4] / ".github/workflows/non-network-rust.yml"
LINUX_JOBS = {"correctness", "behavior", "runtime-platforms"}
GRAPHICS_PACKAGES = {"libgl-dev", "libglx-dev", "libegl-dev"}
EXISTING_PACKAGES = {
    "pkg-config", "libasound2-dev", "libpulse-dev", "libudev-dev", "libdbus-1-dev",
    "libx11-dev", "libxi-dev", "libxrandr-dev", "libxcursor-dev",
    "libxkbcommon-dev", "libwayland-dev", "libvulkan-dev",
}
REQUIRED_JOBS = {"ratchets", "tooling", *LINUX_JOBS}


def workflow_jobs(source: str) -> dict[str, str]:
    body = source.split("\njobs:\n", 1)[1]
    starts = list(re.finditer(r"^  ([a-z][a-z0-9-]*):\s*$", body, re.MULTILINE))
    return {
        match.group(1): body[match.start():starts[index + 1].start() if index + 1 < len(starts) else len(body)]
        for index, match in enumerate(starts)
    }


def linux_dependency_errors(source: str) -> list[str]:
    jobs = workflow_jobs(source)
    errors = []
    installation_jobs = set()
    for name, body in jobs.items():
        steps = re.split(r"^      - name: ", body, flags=re.MULTILINE)[1:]
        installations = [step for step in steps if step.splitlines()[0] == "Install Linux dependencies"]
        if installations:
            installation_jobs.add(name)
        if name not in LINUX_JOBS:
            continue
        if len(installations) != 1:
            errors.append(f"{name}: expected one Linux dependency installation")
            continue
        step = installations[0]
        if name == "runtime-platforms" and "        if: runner.os == 'Linux'\n" not in step:
            errors.append(f"{name}: installation must remain Linux-only")
        shell = step.split("        run: |\n", 1)[-1].replace("\\\n", " ")
        commands = [shlex.split(line, comments=True) for line in shell.splitlines()]
        installs = [tokens[4:] for tokens in commands if tokens[:4] == ["sudo", "apt-get", "install", "-y"]]
        if len(installs) != 1:
            errors.append(f"{name}: expected one apt-get install command")
            continue
        missing = (GRAPHICS_PACKAGES | EXISTING_PACKAGES) - set(installs[0])
        if missing:
            errors.append(f"{name}: missing {' '.join(sorted(missing))}")
    if installation_jobs != LINUX_JOBS:
        errors.append(f"Linux installation jobs must be exactly {sorted(LINUX_JOBS)}")
    return errors


class NonNetworkCiContracts(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.source = WORKFLOW.read_text(encoding="utf-8")

    def test_all_linked_linux_jobs_install_graphics_and_existing_dependencies(self) -> None:
        self.assertEqual([], linux_dependency_errors(self.source))

    def test_each_graphics_package_is_required_in_each_job(self) -> None:
        jobs = workflow_jobs(self.source)
        for name in sorted(LINUX_JOBS):
            for package in sorted(GRAPHICS_PACKAGES):
                with self.subTest(job=name, package=package):
                    changed = jobs[name].replace(f"            {package} \\\n", "", 1)
                    self.assertNotEqual(jobs[name], changed)
                    errors = linux_dependency_errors(self.source.replace(jobs[name], changed, 1))
                    self.assertTrue(any(name in error and package in error for error in errors), errors)

    def test_missing_installation_block_is_rejected(self) -> None:
        for name, body in workflow_jobs(self.source).items():
            if name in LINUX_JOBS:
                with self.subTest(job=name):
                    changed = body.replace("Install Linux dependencies", "Removed installation", 1)
                    self.assertTrue(linux_dependency_errors(self.source.replace(body, changed, 1)))

    def test_runtime_installation_without_linux_guard_is_rejected(self) -> None:
        body = workflow_jobs(self.source)["runtime-platforms"]
        changed = body.replace("        if: runner.os == 'Linux'\n", "", 1)
        self.assertTrue(linux_dependency_errors(self.source.replace(body, changed, 1)))

    def test_linked_test_and_headless_commands_remain_required(self) -> None:
        jobs = workflow_jobs(self.source)
        correctness = " ".join(jobs["correctness"].split())
        self.assertIn(
            "cargo test --locked -p game_engine --features internal --lib "
            "common::thing::thing_factory::tests:: -- --test-threads=1",
            correctness,
        )
        self.assertIn("run: Code/Main/playability_gate.sh full", jobs["behavior"])
        self.assertIn("if: always()", jobs["behavior"])
        self.assertIn("if-no-files-found: error", jobs["behavior"])
        runtime = " ".join(jobs["runtime-platforms"].split())
        self.assertIn("os: [ubuntu-latest, windows-latest, macos-latest]", runtime)
        self.assertIn("cargo check --locked --lib -p generals_main", runtime)
        self.assertIn(
            "cargo check --locked --lib -p game-client-rust "
            "--no-default-features --features platform-native",
            runtime,
        )

    def test_required_aggregate_fails_closed_for_every_supported_job(self) -> None:
        required = workflow_jobs(self.source)["required"]
        needs = re.search(r"^    needs: \[([^\]]+)\]$", required, re.MULTILINE)
        self.assertIsNotNone(needs)
        self.assertEqual(REQUIRED_JOBS, {name.strip() for name in needs.group(1).split(",")})
        self.assertIn("    if: always()\n", required)
        for name in sorted(REQUIRED_JOBS):
            self.assertIn('test "${{ needs.' + name + '.result }}" = "success"', required)
        self.assertNotIn("continue-on-error:", self.source)

    def test_real_main_executable_is_checked_on_every_native_platform(self) -> None:
        runtime = workflow_jobs(self.source)["runtime-platforms"]
        steps = re.split(r"^      - name: ", runtime, flags=re.MULTILINE)[1:]
        selected = [step for step in steps if step.splitlines()[0] == "Check native Main executable"]
        self.assertEqual(1, len(selected))
        self.assertIn("run: cargo check --locked -p generals_main --bin generals", selected[0])
        self.assertNotIn("if:", selected[0])
        self.assertNotIn("--no-default-features", selected[0])

    def test_windows_factory_commands_require_actual_positive_test_counts(self) -> None:
        runtime = workflow_jobs(self.source)["runtime-platforms"]
        steps = re.split(r"^      - name: ", runtime, flags=re.MULTILINE)[1:]
        expected = {
            "Run exact shared factory registry guard on Windows": (
                "engine_factory::tests::startup_factory_registry_does_not_duplicate_cross_platform_fallback -- --exact",
                "1",
            ),
            "Run containing shared factory tests on Windows": (
                "engine_factory::tests::",
                "11",
            ),
        }
        for name, (arguments, count) in expected.items():
            with self.subTest(step=name):
                selected = [step for step in steps if step.splitlines()[0] == name]
                self.assertEqual(1, len(selected))
                step = selected[0]
                self.assertIn("if: runner.os == 'Windows'", step)
                self.assertIn("shell: pwsh", step)
                self.assertIn(f"cargo test --locked -p generals_main --lib {arguments}", step)
                self.assertIn("2>&1 | Tee-Object -Variable factoryOutput", step)
                self.assertIn("if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }", step)
                self.assertIn(f"if (-not ($factoryOutput -match '^test result: ok\\. {count} passed; 0 failed;'))", step)
                self.assertIn("throw 'Expected", step)


if __name__ == "__main__":
    unittest.main()
