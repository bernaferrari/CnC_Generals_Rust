from pathlib import Path
import sys
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import port_dashboard


class PortDashboardTests(unittest.TestCase):
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
            "gates": [
                {"name": name, "passed": True} for name, _args in port_dashboard.QUICK_GATES
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
                {
                    "name": "cpp_rust_randomvalue_differential",
                    "passed": True,
                }
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


if __name__ == "__main__":
    unittest.main()
