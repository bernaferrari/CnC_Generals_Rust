import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import port_dashboard


class PortDashboardTests(unittest.TestCase):
    @staticmethod
    def _passing_gate(name: str, args: list[str]) -> dict:
        is_test = args[:2] == ["cargo", "test"] and "--no-run" not in args
        harnesses = port_dashboard.selected_test_harness_count(args) if is_test else None
        harnesses = harnesses or (1 if is_test else 0)
        return {
            "name": name,
            "command": list(args),
            "exit_code": 0,
            "passed": True,
            "tests_passed": harnesses if is_test else None,
            "tests_failed": 0 if is_test else None,
            "test_result_lines": [
                "test result: ok. 1 passed; 0 failed; 0 ignored;"
                for _ in range(harnesses)
            ],
            "output_tail": [],
        }

    def test_full_gate_contract_covers_each_behavior_layer(self) -> None:
        names = {name for name, _args in port_dashboard.FULL_GATES}
        self.assertTrue(
            {
                "main_behavior_modules",
                "deterministic_frame_trace",
                "save_load_integration",
                "ui_state_integration",
                "playable_smoke_integration",
                "cpp_rust_randomvalue_differential",
                "golden_skirmish",
                "ai_skirmish",
                "map_frame",
            }
            <= names
        )

    def test_confidence_ladder_never_conflates_candidates_with_verified_behavior(self) -> None:
        ladder = port_dashboard.confidence_ladder(
            {
                "required_translation_units": 100,
                "reachable_implementation_candidates": 80,
                "reviewed_path_implementations": 20,
                "behavior_verified": 3,
                "blockers_by_kind": {"unreviewed_symbol_ownership": 95},
            }
        )
        self.assertEqual(80.0, ladder["reachable_candidate"]["percent"])
        self.assertEqual(20.0, ladder["reviewed_path"]["percent"])
        self.assertEqual(5.0, ladder["reviewed_symbol_ownership"]["percent"])
        self.assertEqual(3.0, ladder["behavior_verified"]["percent"])

    def test_unverified_inventory_and_missing_evidence_cannot_pass(self) -> None:
        manifest = {
            "summary": {
                "required_translation_units": 10,
                "deferred_network_translation_units": 2,
                "strict_blockers": 3,
            },
            "metric_contract": {"inventory_is_behavior_parity": False},
        }
        with (
            mock.patch.object(
                port_dashboard,
                "git_metadata",
                return_value={"commit_sha": "abc", "commit_timestamp": "now"},
            ),
            mock.patch.object(port_dashboard, "worktree_digest", return_value="tree"),
            mock.patch.object(port_dashboard, "beads_status", return_value={"available": False}),
        ):
            dashboard = port_dashboard.build_dashboard(Path("."), manifest, None)
        self.assertEqual("fail", dashboard["grades"]["inventory"])
        self.assertEqual("unknown", dashboard["grades"]["build_and_tests"])
        self.assertEqual("missing", dashboard["grades"]["cpp_rust_differential"])

    def test_only_current_complete_command_evidence_passes_gate_grade(self) -> None:
        manifest = {
            "summary": {
                "required_translation_units": 10,
                "deferred_network_translation_units": 2,
                "strict_blockers": 0,
            },
            "metric_contract": {},
        }
        evidence = {
            "commit_sha": "abc",
            "worktree_digest": "tree",
            "profile": "quick",
            "gates": [self._passing_gate(name, args) for name, args in port_dashboard.QUICK_GATES],
        }
        with (
            mock.patch.object(
                port_dashboard,
                "git_metadata",
                return_value={"commit_sha": "abc", "commit_timestamp": "now"},
            ),
            mock.patch.object(port_dashboard, "worktree_digest", return_value="tree"),
            mock.patch.object(port_dashboard, "beads_status", return_value={"available": True}),
        ):
            dashboard = port_dashboard.build_dashboard(Path("."), manifest, evidence)
        self.assertEqual("pass", dashboard["grades"]["inventory"])
        self.assertEqual("pass", dashboard["grades"]["build_and_tests"])
        self.assertEqual("unknown", dashboard["grades"]["headless_behavior"])

    def test_component_differential_grade_requires_current_passing_command(self) -> None:
        manifest = {
            "summary": {
                "required_translation_units": 1,
                "deferred_network_translation_units": 0,
                "strict_blockers": 1,
            },
            "metric_contract": {},
        }
        evidence = {
            "commit_sha": "abc",
            "worktree_digest": "tree",
            "profile": "full",
            "gates": [
                self._passing_gate(name, args)
                for name, args in port_dashboard.QUICK_GATES + port_dashboard.FULL_GATES
            ],
        }
        with (
            mock.patch.object(
                port_dashboard,
                "git_metadata",
                return_value={"commit_sha": "abc", "commit_timestamp": "now"},
            ),
            mock.patch.object(port_dashboard, "worktree_digest", return_value="tree"),
            mock.patch.object(port_dashboard, "beads_status", return_value={"available": True}),
        ):
            dashboard = port_dashboard.build_dashboard(Path("."), manifest, evidence)
        self.assertEqual("component-pass", dashboard["grades"]["cpp_rust_differential"])
        self.assertFalse(dashboard["differential_scope"]["full_game_loop"])

    def test_runner_records_every_selected_gate_and_rejects_failed_command(self) -> None:
        calls = []

        def fake_command(_repo, args, *, check=True):
            calls.append(list(args))
            if args[0] == "cargo" and args[1] == "check":
                return subprocess.CompletedProcess(args, 7, "compiler output", "failure detail")
            is_test = args[:2] == ["cargo", "test"] and "--no-run" not in args
            output = "test result: ok. 2 passed; 0 failed; 0 ignored;" if is_test else "ok"
            return subprocess.CompletedProcess(args, 0, output, "")

        with (
            mock.patch.object(port_dashboard, "command", side_effect=fake_command),
            mock.patch.object(port_dashboard, "git_metadata", return_value={"commit_sha": "abc", "commit_timestamp": "now"}),
            mock.patch.object(port_dashboard, "worktree_digest", return_value="tree"),
        ):
            evidence = port_dashboard.run_gates(Path("repo"), Path("rust"), full=False)

        self.assertEqual(len(port_dashboard.QUICK_GATES), len(calls))
        self.assertEqual([name for name, _ in port_dashboard.QUICK_GATES], [g["name"] for g in evidence["gates"]])
        self.assertEqual(7, evidence["gates"][0]["exit_code"])
        self.assertFalse(evidence["gates"][0]["passed"])
        self.assertIn("failure detail", evidence["gates"][0]["output_tail"])
        self.assertTrue(port_dashboard.gate_evidence_errors(evidence, "quick"))

    def test_evidence_contract_rejects_wrong_profile_commands_partial_and_zero_tests(self) -> None:
        evidence = {
            "commit_sha": "abc",
            "worktree_digest": "tree",
            "profile": "quick",
            "gates": [self._passing_gate(name, args) for name, args in port_dashboard.QUICK_GATES],
        }
        self.assertEqual([], port_dashboard.gate_evidence_errors(evidence, "quick"))
        self.assertTrue(port_dashboard.gate_evidence_errors(evidence, "full"))

        partial = dict(evidence, gates=evidence["gates"][:-1])
        self.assertTrue(any("incomplete gate count" in item for item in port_dashboard.gate_evidence_errors(partial)))

        wrong_command = dict(evidence, gates=[dict(g) for g in evidence["gates"]])
        wrong_command["gates"][0]["command"] = ["cargo", "check"]
        self.assertTrue(any("command does not match" in item for item in port_dashboard.gate_evidence_errors(wrong_command)))

        reordered = dict(evidence, gates=list(reversed(evidence["gates"])))
        self.assertTrue(any("name mismatch" in item for item in port_dashboard.gate_evidence_errors(reordered)))
        duplicated = dict(evidence, gates=[dict(g) for g in evidence["gates"]])
        duplicated["gates"][1] = dict(duplicated["gates"][0])
        self.assertTrue(any("name mismatch" in item for item in port_dashboard.gate_evidence_errors(duplicated)))

        zero_tests = dict(evidence, gates=[dict(g) for g in evidence["gates"]])
        test_gate = next(g for g in zero_tests["gates"] if g["tests_passed"] is not None)
        test_gate.update(tests_passed=0, test_result_lines=["test result: ok. 0 passed; 0 failed; 0 ignored;"])
        self.assertTrue(any("no positive executed-test" in item for item in port_dashboard.gate_evidence_errors(zero_tests)))

        inconsistent = dict(evidence, gates=[dict(g) for g in evidence["gates"]])
        test_gate = next(g for g in inconsistent["gates"] if g["tests_passed"] is not None)
        test_gate["tests_failed"] = 1
        self.assertTrue(any("failed-test count does not match" in item for item in port_dashboard.gate_evidence_errors(inconsistent)))

        malformed = dict(evidence, gates=[dict(g) for g in evidence["gates"]])
        test_gate = next(g for g in malformed["gates"] if g["tests_passed"] is not None)
        test_gate["tests_passed"] = True
        test_gate["test_result_lines"] = "test result: ok. 1 passed; 0 failed;"
        self.assertTrue(any("malformed test result lines" in item for item in port_dashboard.gate_evidence_errors(malformed)))

        invalid_status = dict(evidence, gates=[dict(g) for g in evidence["gates"]])
        invalid_status["gates"][0]["exit_code"] = False
        self.assertTrue(any("integer status" in item for item in port_dashboard.gate_evidence_errors(invalid_status)))

        multi_harness_zero = dict(evidence, profile="full", gates=[self._passing_gate(name, args) for name, args in port_dashboard.QUICK_GATES + port_dashboard.FULL_GATES])
        multi_gate = next(g for g in multi_harness_zero["gates"] if g["name"] == "ui_state_integration")
        multi_gate["test_result_lines"] = [
            "test result: ok. 3 passed; 0 failed; 0 ignored;",
            "test result: ok. 0 passed; 0 failed; 0 ignored;",
        ]
        multi_gate["tests_passed"] = 3
        self.assertTrue(any("selected harness with no passing tests" in item for item in port_dashboard.gate_evidence_errors(multi_harness_zero)))

    def test_run_gates_main_returns_failure_after_writing_both_artifacts(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repo = Path(temporary)
            (repo / "PORT_PROVENANCE_MANIFEST.json").write_text(
                json.dumps({"summary": {"required_translation_units": 1, "deferred_network_translation_units": 0, "strict_blockers": 0}, "metric_contract": {}}),
                encoding="utf-8",
            )
            evidence_path = repo / "evidence.json"
            dashboard_path = repo / "dashboard.json"
            args = type("Args", (), {"repo_root": repo, "evidence": evidence_path, "output": dashboard_path, "run_gates": "quick"})()
            calls = []

            def fake_command(_repo, command, *, check=True):
                calls.append(list(command))
                if len(calls) == 1:
                    raise FileNotFoundError("cargo executable missing")
                is_test = command[:2] == ["cargo", "test"] and "--no-run" not in command
                count = port_dashboard.selected_test_harness_count(command) or 1
                output = "\n".join(
                    "test result: ok. 1 passed; 0 failed; 0 ignored;"
                    for _ in range(count)
                ) if is_test else "ok"
                return subprocess.CompletedProcess(command, 0, output, "")

            with (
                mock.patch.object(port_dashboard, "parse_args", return_value=args),
                mock.patch.object(port_dashboard, "command", side_effect=fake_command),
                mock.patch.object(port_dashboard, "git_metadata", return_value={"commit_sha": "abc", "commit_timestamp": "now"}),
                mock.patch.object(port_dashboard, "worktree_digest", return_value="tree"),
                mock.patch.object(port_dashboard, "quality_status", return_value={}),
                mock.patch.object(port_dashboard, "beads_status", return_value={"available": False}),
            ):
                status = port_dashboard.main()

            self.assertEqual(1, status)
            self.assertEqual(len(port_dashboard.QUICK_GATES), len(calls))
            self.assertTrue(evidence_path.is_file())
            self.assertTrue(dashboard_path.is_file())
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            self.assertEqual(len(port_dashboard.QUICK_GATES), len(evidence["gates"]))
            self.assertIn("could not start command", evidence["gates"][0]["output_tail"][0])

    def test_valid_quick_and_full_runs_exit_zero(self) -> None:
        for profile in ("quick", "full"):
            with self.subTest(profile=profile), tempfile.TemporaryDirectory() as temporary:
                repo = Path(temporary)
                (repo / "PORT_PROVENANCE_MANIFEST.json").write_text(
                    json.dumps({"summary": {"required_translation_units": 1, "deferred_network_translation_units": 0, "strict_blockers": 0}, "metric_contract": {}}),
                    encoding="utf-8",
                )
                args = type(
                    "Args",
                    (),
                    {"repo_root": repo, "evidence": repo / "evidence.json", "output": repo / "dashboard.json", "run_gates": profile},
                )()

                def fake_command(_repo, command, *, check=True):
                    is_test = command[:2] == ["cargo", "test"] and "--no-run" not in command
                    count = port_dashboard.selected_test_harness_count(command) or 1
                    output = "\n".join(
                        "test result: ok. 1 passed; 0 failed; 0 ignored;"
                        for _ in range(count)
                    ) if is_test else "ok"
                    return subprocess.CompletedProcess(command, 0, output, "")

                with (
                    mock.patch.object(port_dashboard, "parse_args", return_value=args),
                    mock.patch.object(port_dashboard, "command", side_effect=fake_command),
                    mock.patch.object(port_dashboard, "git_metadata", return_value={"commit_sha": "abc", "commit_timestamp": "now"}),
                    mock.patch.object(port_dashboard, "worktree_digest", return_value="tree"),
                    mock.patch.object(port_dashboard, "quality_status", return_value={}),
                    mock.patch.object(port_dashboard, "beads_status", return_value={"available": False}),
                ):
                    self.assertEqual(0, port_dashboard.main())

                evidence = json.loads(args.evidence.read_text(encoding="utf-8"))
                self.assertEqual(profile, evidence["profile"])
                self.assertEqual(len(port_dashboard.selected_gates(profile)), len(evidence["gates"]))

    def test_report_only_stays_successful_when_current_gate_evidence_is_bad(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repo = Path(temporary)
            (repo / "PORT_PROVENANCE_MANIFEST.json").write_text(
                json.dumps({"summary": {"required_translation_units": 1, "deferred_network_translation_units": 0, "strict_blockers": 0}, "metric_contract": {}}),
                encoding="utf-8",
            )
            evidence_path = repo / "evidence.json"
            evidence_path.write_text(
                json.dumps({"commit_sha": "abc", "worktree_digest": "tree", "profile": "quick", "gates": []}),
                encoding="utf-8",
            )
            args = type("Args", (), {"repo_root": repo, "evidence": evidence_path, "output": repo / "dashboard.json", "run_gates": None})()
            with (
                mock.patch.object(port_dashboard, "parse_args", return_value=args),
                mock.patch.object(port_dashboard, "git_metadata", return_value={"commit_sha": "abc", "commit_timestamp": "now"}),
                mock.patch.object(port_dashboard, "worktree_digest", return_value="tree"),
                mock.patch.object(port_dashboard, "quality_status", return_value={}),
                mock.patch.object(port_dashboard, "beads_status", return_value={"available": False}),
            ):
                self.assertEqual(0, port_dashboard.main())

    def test_headless_shell_propagates_runner_failure(self) -> None:
        source = Path(__file__).resolve().parents[1] / "playability_gate.sh"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            script = root / "GeneralsRust/Code/Main/playability_gate.sh"
            script.parent.mkdir(parents=True)
            shutil.copy2(source, script)
            shim_dir = root / "bin"
            shim_dir.mkdir()
            shim = shim_dir / "python3"
            shim.write_text(
                "#!/bin/sh\n"
                "case \" $* \" in\n"
                "  *port_dashboard.py*) echo 'simulated gate failure' >&2; exit 19 ;;\n"
                "  *) exit 0 ;;\n"
                "esac\n",
                encoding="utf-8",
            )
            shim.chmod(0o755)
            environment = dict(os.environ, PATH=f"{shim_dir}:{os.environ['PATH']}")
            result = subprocess.run(["bash", str(script), "quick"], capture_output=True, text=True, env=environment, timeout=10)
            self.assertNotEqual(0, result.returncode)
            self.assertIn("simulated gate failure", result.stderr)

    def test_actual_runner_process_propagates_gate_failure_and_keeps_full_evidence(self) -> None:
        script = Path(__file__).resolve().with_name("port_dashboard.py")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "GeneralsRust").mkdir()
            (root / "PORT_PROVENANCE_MANIFEST.json").write_text(
                json.dumps({"summary": {"required_translation_units": 1, "deferred_network_translation_units": 0, "strict_blockers": 0}, "metric_contract": {}}),
                encoding="utf-8",
            )
            shim_dir = root / "bin"
            shim_dir.mkdir()
            shims = {
                "cargo": """#!/bin/sh
case "$1" in
  check) echo 'simulated compiler failure' >&2; exit 7 ;;
  test)
    shift
    case " $* " in *" --no-run "*) exit 0 ;; esac
    count=0
    for arg in "$@"; do
      if [ "$arg" = "--test" ]; then count=$((count + 1)); fi
    done
    if [ "$count" -eq 0 ]; then count=1; fi
    while [ "$count" -gt 0 ]; do
      echo 'test result: ok. 1 passed; 0 failed; 0 ignored;'
      count=$((count - 1))
    done
    exit 0 ;;
  *) exit 0 ;;
esac
""",
                "git": """#!/bin/sh
case "$1" in
  rev-parse) echo abc ;;
  show) echo 2026-10-08T00:00:00+00:00 ;;
  diff) exit 0 ;;
  ls-files) exit 0 ;;
  *) exit 0 ;;
esac
""",
                "bd": """#!/bin/sh
case "$1" in
  stats) echo '{}' ;;
  list) echo '[]' ;;
  *) exit 0 ;;
esac
""",
                "python3": """#!/bin/sh
case " $* " in
  *run_cpp_rust_differential.py*) exit 0 ;;
  *check_rust_loc.py*|*check_unsafe_contracts.py*) echo '{"passed": true}' ;;
  *) echo 'unexpected python command' >&2; exit 2 ;;
esac
""",
            }
            for name, content in shims.items():
                path = shim_dir / name
                path.write_text(content, encoding="utf-8")
                path.chmod(0o755)

            evidence_path = root / "evidence.json"
            dashboard_path = root / "dashboard.json"
            environment = dict(os.environ, PATH=f"{shim_dir}:{os.environ['PATH']}")
            result = subprocess.run(
                [
                    sys.executable,
                    str(script),
                    "--repo-root",
                    str(root),
                    "--run-gates",
                    "quick",
                    "--evidence",
                    str(evidence_path),
                    "--output",
                    str(dashboard_path),
                ],
                capture_output=True,
                text=True,
                env=environment,
                timeout=20,
            )

            self.assertEqual(1, result.returncode, result.stdout + result.stderr)
            self.assertTrue(evidence_path.is_file())
            self.assertTrue(dashboard_path.is_file())
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            self.assertEqual("quick", evidence["profile"])
            self.assertEqual(
                [name for name, _args in port_dashboard.QUICK_GATES],
                [gate["name"] for gate in evidence["gates"]],
            )
            self.assertEqual(len(port_dashboard.QUICK_GATES), len(evidence["gates"]))
            self.assertEqual(7, evidence["gates"][0]["exit_code"])
            self.assertIn("simulated compiler failure", evidence["gates"][0]["output_tail"])
            dashboard = json.loads(dashboard_path.read_text(encoding="utf-8"))
            self.assertEqual("fail", dashboard["grades"]["build_and_tests"])


if __name__ == "__main__":
    unittest.main()
