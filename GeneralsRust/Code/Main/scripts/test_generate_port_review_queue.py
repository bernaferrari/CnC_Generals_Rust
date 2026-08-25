from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))

import generate_port_review_queue as queue


def entry(index: int, *, network: bool = False, reviewed: bool = False) -> dict:
    area = "GameNetwork" if network else "GameLogic/AI"
    source = f"GeneralsMD/Code/GameEngine/Source/{area}/Unit{index:02}.cpp"
    return {
        "id": f"game_engine:{area}/Unit{index:02}.cpp",
        "scope": "game_engine",
        "inventory_class": "deferred_network" if network else "required_runtime",
        "unit_kind": "translation_unit",
        "source": {"path": source, "sha256": f"hash-{index}"},
        "mapping": {
            "review_state": "reviewed" if reviewed else "unreviewed",
            "destinations": [
                {
                    "path": f"GeneralsRust/Code/Main/src/unit_{index}.rs",
                    "classification": "implementation",
                    "cargo_reachable": True,
                }
            ],
        },
        "blockers": [] if reviewed or network else ["unreviewed_mapping"],
    }


class PortReviewQueueTests(unittest.TestCase):
    def fixture(self, count: int = 23) -> tuple[Path, dict]:
        root = Path(tempfile.mkdtemp())
        entries = [entry(index) for index in range(count)]
        entries += [entry(100, network=True), entry(101, reviewed=True)]
        for item in entries:
            path = root / item["source"]["path"]
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(f"void Unit{item['id'].split('Unit')[-1][:-4]}::update() {{}}\n")
        return root, {"input_digest": "fixture", "entries": entries}

    def test_packets_are_bounded_complete_disjoint_and_exclude_network_reviewed(self) -> None:
        root, manifest = self.fixture()
        result = queue.build_queue(root, manifest, packet_size=10)
        summary = result["summary"]
        self.assertEqual(23, summary["unreviewed_required_translation_units"])
        self.assertEqual(23, summary["unique_units"])
        self.assertEqual(10, summary["maximum_packet_size"])
        self.assertEqual(3, summary["packets"])
        paths = [
            unit["source"]["path"]
            for packet in result["packets"]
            for unit in packet["units"]
        ]
        self.assertEqual(len(paths), len(set(paths)))
        self.assertFalse(any("GameNetwork" in path for path in paths))
        self.assertFalse(any("Unit101" in path for path in paths))
        self.assertTrue(all(unit["source"]["symbols"] for packet in result["packets"] for unit in packet["units"]))
        selected = queue.select_packet(result, result["packets"][0]["id"])
        self.assertEqual(result["packets"][0], selected)
        with self.assertRaisesRegex(ValueError, "unknown provenance packet"):
            queue.select_packet(result, "missing")

    def test_rerun_is_deterministic_and_rejects_oversized_packet_setting(self) -> None:
        root, manifest = self.fixture(4)
        self.assertEqual(
            queue.build_queue(root, manifest, packet_size=3),
            queue.build_queue(root, manifest, packet_size=3),
        )
        with self.assertRaisesRegex(ValueError, "1..=20"):
            queue.build_queue(root, manifest, packet_size=21)


if __name__ == "__main__":
    unittest.main()
