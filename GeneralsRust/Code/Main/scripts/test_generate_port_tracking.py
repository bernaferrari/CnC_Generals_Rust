from __future__ import annotations

import sys
import tempfile
import unittest
from collections import defaultdict
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import generate_port_tracking as tracking  # noqa: E402


EXPECTED_GUI_HINTS = {
    ("Source", "GameClient/GUI/ControlBar/ControlBar.cpp"): "GameClient/src/gui/control_bar/control_bar_impl/mod.rs",
    ("Source", "GameClient/GUI/ControlBar/ControlBarScheme.cpp"): "Common/src/common/ini/ini_control_bar_scheme.rs",
    ("Source", "GameClient/GUI/DisconnectMenu/DisconnectMenu.cpp"): "GameClient/src/gui/menus/disconnect_menu.rs",
    ("Source", "GameClient/GUI/EstablishConnectionsMenu/EstablishConnectionsMenu.cpp"): "GameClient/src/gui/menus/establish_connections_menu.rs",
    ("Source", "GameClient/GUI/GameWindow.cpp"): "GameClient/src/gui/game_window/mod.rs",
    ("Source", "GameClient/GUI/LoadScreen.cpp"): "GameClient/src/gui/load_screen/mod.rs",
    ("Source", "GameClient/GUI/Shell/ShellMenuScheme.cpp"): "GameClient/src/gui/shell/base/scheme.rs",
    ("Include", "GameClient/ControlBar.h"): "GameClient/src/gui/control_bar/control_bar_impl/mod.rs",
    ("Include", "GameClient/ControlBarScheme.h"): "Common/src/common/ini/ini_control_bar_scheme.rs",
    ("Include", "GameClient/DisconnectMenu.h"): "GameClient/src/gui/menus/disconnect_menu.rs",
    ("Include", "GameClient/EstablishConnectionsMenu.h"): "GameClient/src/gui/menus/establish_connections_menu.rs",
    ("Include", "GameClient/GameWindow.h"): "GameClient/src/gui/game_window/mod.rs",
    ("Include", "GameClient/LoadScreen.h"): "GameClient/src/gui/load_screen/mod.rs",
    ("Include", "GameClient/ShellMenuScheme.h"): "GameClient/src/gui/shell/base/scheme.rs",
}


class GeneratePortTrackingTests(unittest.TestCase):
    def test_upgrade_hint_targets_the_canonical_ini_callback(self) -> None:
        key = ("Source", "Common/INI/INIUpgrade.cpp")
        destination = "Common/src/common/ini/ini.rs"
        self.assertEqual(tracking.MANUAL_CPP_TO_RUST[key], destination)
        source = (SCRIPT_DIR.parents[2] / "Code/GameEngine" / destination).read_text(
            encoding="utf-8"
        )
        self.assertIn("center.parse_upgrade_definition(ini)", source)

    def test_missing_reports_end_with_one_newline(self) -> None:
        for subsystems in [(), ("Common",), ("Common", "GameLogic")]:
            with self.subTest(subsystems=subsystems), tempfile.TemporaryDirectory() as temp:
                path = Path(temp) / "missing.txt"
                rows = [
                    tracking.MappingRow(
                        kind="Source",
                        source_rel=Path(subsystem) / "Missing.cpp",
                        subsystem=subsystem,
                        status=tracking.SOURCE_STATUS_MISSING,
                        mapped_rel=None,
                    )
                    for subsystem in subsystems
                ]
                tracking.write_missing(path, rows)
                report = path.read_text(encoding="utf-8")
                self.assertTrue(report.endswith("\n"))
                self.assertFalse(report.endswith("\n\n"))
                for subsystem in subsystems:
                    self.assertIn(f"[{subsystem}]\nSource missing:\n", report)
                if len(subsystems) > 1:
                    self.assertIn("Include missing:\n\n[GameLogic]", report)

    def test_kind_of_case_correct_legacy_hint_wins_over_basename_fallback(self) -> None:
        # These are filename hints, not implementation or behavior evidence.
        destination = Path("Common/src/System/kind_of.rs")
        keys = (
            ("Source", "Common/System/KindOf.cpp"),
            ("Include", "Common/KindOf.h"),
            ("Include", "Common/System/KindOf.h"),
        )
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            rust_root = root / "rust"
            compatibility = rust_root / destination
            fallback = rust_root / "GameLogic/src/common/types/kindof.rs"
            for path in (compatibility, fallback):
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("// Filename candidate only.\n", encoding="utf-8")

            by_stem: dict[str, list[Path]] = defaultdict(list)
            by_normalized: dict[str, list[Path]] = defaultdict(list)
            for path in (compatibility, fallback):
                by_stem[path.stem.lower()].append(path)
                by_normalized[tracking.normalize_name(path.stem)].append(path)

            for kind, source in keys:
                cpp_file = root / kind / source
                cpp_file.parent.mkdir(parents=True, exist_ok=True)
                cpp_file.write_text("// Legacy filename only.\n", encoding="utf-8")

            for kind in ("Source", "Include"):
                rows = tracking.build_mapping_rows(
                    kind, root / kind, rust_root, by_stem, by_normalized
                )
                self.assertEqual(sum(key[0] == kind for key in keys), len(rows))
                for row in rows:
                    with self.subTest(kind=kind, source=row.source_rel.as_posix()):
                        self.assertEqual(destination, row.mapped_rel)
                        self.assertEqual(tracking.SOURCE_STATUS_FOUND, row.status)
                        self.assertEqual(
                            destination.as_posix(),
                            tracking.MANUAL_CPP_TO_RUST[(kind, row.source_rel.as_posix())],
                        )

    def test_catalogue_paths_are_excluded_from_candidate_index(self) -> None:
        self.assertTrue(tracking.is_gpui_catalogue_path(Path("GameClient/gui/src/gui/game_window.rs")))
        self.assertFalse(tracking.is_gpui_catalogue_path(Path("GameClient/src/gui/game_window/mod.rs")))

    def test_all_legacy_gui_hints_target_reachable_production_files(self) -> None:
        self.assertEqual(
            {key: value for key, value in tracking.MANUAL_CPP_TO_RUST.items() if key in EXPECTED_GUI_HINTS},
            EXPECTED_GUI_HINTS,
        )

    def test_ai_states_hint_targets_the_canonical_runtime_module(self) -> None:
        key = ("Source", "GameLogic/AI/AIStates.cpp")
        destination = "GameLogic/src/ai/states/mod.rs"
        self.assertEqual(tracking.MANUAL_CPP_TO_RUST[key], destination)
        engine_root = SCRIPT_DIR.parents[2] / "Code/GameEngine"
        self.assertTrue((engine_root / destination).is_file())
        self.assertFalse((engine_root / "GameLogic/src/ai/ai_states").exists())

    def test_production_hint_destinations_exist_in_repository(self) -> None:
        engine_root = SCRIPT_DIR.parents[2] / "Code/GameEngine"
        for hint in EXPECTED_GUI_HINTS.values():
            with self.subTest(hint=hint):
                self.assertTrue((engine_root / hint).is_file())

    def test_explicit_production_hint_wins_over_same_stem_gpui_sample(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            cpp_root = root / "cpp"
            rust_root = root / "rust"
            cpp_file = cpp_root / "GameClient/GUI/GameWindow.cpp"
            production = rust_root / EXPECTED_GUI_HINTS[("Source", "GameClient/GUI/GameWindow.cpp")]
            sample = rust_root / "GameClient/gui/src/gui/game_window.rs"
            cpp_file.parent.mkdir(parents=True)
            cpp_file.write_text("void GameWindow::update() {}\n", encoding="utf-8")
            production.parent.mkdir(parents=True)
            production.write_text("pub struct GameWindow;\n", encoding="utf-8")
            sample.parent.mkdir(parents=True)
            sample.write_text("pub struct GameWindowPort;\n", encoding="utf-8")

            candidates = [production, sample]
            by_stem: dict[str, list[Path]] = defaultdict(list)
            by_normalized: dict[str, list[Path]] = defaultdict(list)
            for path in candidates:
                relative = path.relative_to(rust_root)
                if tracking.is_gpui_catalogue_path(relative):
                    continue
                by_stem[path.stem.lower()].append(path)
                by_normalized[tracking.normalize_name(path.stem)].append(path)

            rows = tracking.build_mapping_rows("Source", cpp_root, rust_root, by_stem, by_normalized)

            self.assertEqual(1, len(rows))
            self.assertEqual(Path("GameClient/src/gui/game_window/mod.rs"), rows[0].mapped_rel)
            self.assertEqual(tracking.SOURCE_STATUS_FOUND, rows[0].status)


if __name__ == "__main__":
    unittest.main()
