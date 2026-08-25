from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import generate_port_provenance as provenance  # noqa: E402


class ProvenanceFixture:
    def __init__(self, root: Path) -> None:
        self.root = root

    def write(self, relative: str, content: str) -> Path:
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
        return path

    def minimal_game_engine(self) -> None:
        self.write(
            "GeneralsRust/Code/GameEngine/GameClient/Cargo.toml",
            '[package]\nname = "fixture-client"\nversion = "0.1.0"\n',
        )
        self.write(
            "GeneralsRust/Code/GameEngine/GameClient/src/lib.rs",
            "pub mod foo;\npub mod bar;\n",
        )
        self.write(
            "GeneralsRust/Code/GameEngine/GameClient/src/foo/mod.rs",
            'include!("part.rs");\n#[cfg(test)]\nmod tests;\n',
        )
        self.write(
            "GeneralsRust/Code/GameEngine/GameClient/src/foo/part.rs",
            "pub struct FooState;\npub fn update_foo() {}\n",
        )
        self.write(
            "GeneralsRust/Code/GameEngine/GameClient/src/foo/tests.rs",
            "#[test]\nfn foo_works() {}\n",
        )
        self.write(
            "GeneralsRust/Code/GameEngine/GameClient/src/bar.rs",
            "//! Telemetry-only helper, not a C++ behavior port.\npub fn emit() {}\n",
        )
        self.write(
            "GeneralsRust/Code/GameEngine/GameClient/tests/baz.rs",
            "#[test]\nfn baz_name_only() {}\n",
        )
        self.write(
            "GeneralsMD/Code/GameEngine/Source/GameClient/Foo.cpp",
            "void Foo::updateFoo() {}\n",
        )
        self.write(
            "GeneralsMD/Code/GameEngine/Source/GameClient/Bar.cpp",
            "void Bar::update() {}\n",
        )
        self.write(
            "GeneralsMD/Code/GameEngine/Source/GameClient/Baz.cpp",
            "void Baz::update() {}\n",
        )
        self.write(
            "GeneralsMD/Code/GameEngine/Source/GameNetwork/Net.cpp",
            "void Net::update() {}\n",
        )


class GeneratePortProvenanceTests(unittest.TestCase):
    def build_fixture(self) -> tuple[tempfile.TemporaryDirectory[str], ProvenanceFixture]:
        temporary = tempfile.TemporaryDirectory()
        fixture = ProvenanceFixture(Path(temporary.name))
        fixture.minimal_game_engine()
        return temporary, fixture

    @staticmethod
    def entry(manifest: dict[str, object], suffix: str) -> dict[str, object]:
        return next(
            entry
            for entry in manifest["entries"]
            if entry["source"]["path"].endswith(suffix)
        )

    def test_split_module_expands_to_reachable_fragments_and_keeps_tests_non_implementation(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)

        manifest = provenance.build_manifest(fixture.root)
        foo = self.entry(manifest, "Foo.cpp")
        destinations = foo["mapping"]["destinations"]

        self.assertEqual("split_inferred", foo["mapping"]["mode"])
        self.assertEqual(3, len(destinations))
        self.assertEqual(
            {"implementation", "test"},
            {destination["classification"] for destination in destinations},
        )
        self.assertTrue(all(destination["cargo_reachable"] for destination in destinations))
        self.assertEqual(
            "reachable_implementation_candidate",
            foo["mapping"]["candidate_status"],
        )
        self.assertIn("unreviewed_mapping", foo["blockers"])

    def test_telemetry_and_test_candidates_cannot_satisfy_implementation_coverage(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)

        manifest = provenance.build_manifest(fixture.root)
        bar = self.entry(manifest, "Bar.cpp")
        baz = self.entry(manifest, "Baz.cpp")

        self.assertEqual("telemetry", bar["mapping"]["destinations"][0]["classification"])
        self.assertEqual("no_reachable_implementation", bar["mapping"]["candidate_status"])
        self.assertEqual("test", baz["mapping"]["destinations"][0]["classification"])
        self.assertEqual("no_reachable_implementation", baz["mapping"]["candidate_status"])

    def test_network_units_are_explicitly_deferred_not_silently_omitted(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)

        manifest = provenance.build_manifest(fixture.root)
        network = self.entry(manifest, "Net.cpp")

        self.assertEqual("deferred_network", network["inventory_class"])
        self.assertEqual([], network["blockers"])
        self.assertEqual(1, manifest["summary"]["deferred_network_translation_units"])

    def test_explicit_split_mapping_receives_path_review_credit_but_not_behavior_credit(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        source = "GeneralsMD/Code/GameEngine/Source/GameClient/Foo.cpp"
        destination = "GeneralsRust/Code/GameEngine/GameClient/src/foo/mod.rs"

        manifest = provenance.build_manifest(
            fixture.root, {source: (destination,)}
        )
        foo = self.entry(manifest, "Foo.cpp")

        self.assertEqual("explicit", foo["mapping"]["mode"])
        self.assertEqual("reviewed", foo["mapping"]["review_state"])
        self.assertTrue(foo["mapping"]["reviewed_path_implementation"])
        self.assertIn("unreviewed_symbol_ownership", foo["blockers"])
        self.assertEqual("not_verified", foo["behavior"]["status"])

    def test_generation_is_deterministic_and_status_names_each_metric(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        output_a = fixture.root / "out-a"
        output_b = fixture.root / "out-b"

        first = provenance.generate(fixture.root, output_a)
        second = provenance.generate(fixture.root, output_b)

        self.assertEqual(first, second)
        self.assertEqual(
            (output_a / "PORT_PROVENANCE_MANIFEST.json").read_bytes(),
            (output_b / "PORT_PROVENANCE_MANIFEST.json").read_bytes(),
        )
        state = (output_a / "PORT_STATE.txt").read_text(encoding="utf-8")
        self.assertIn("ReachableImplementationCandidatePercent=", state)
        self.assertIn("ReviewedPathImplementationPercent=", state)
        self.assertIn("BehaviorVerifiedPercent=0.00", state)
        parsed = json.loads(
            (output_a / "PORT_PROVENANCE_MANIFEST.json").read_text(encoding="utf-8")
        )
        self.assertFalse(parsed["metric_contract"]["inventory_is_behavior_parity"])

    def test_stale_explicit_mapping_is_a_strict_blocker(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        source = "GeneralsMD/Code/GameEngine/Source/GameClient/Foo.cpp"
        missing = "GeneralsRust/Code/GameEngine/GameClient/src/foo/removed.rs"

        manifest = provenance.build_manifest(fixture.root, {source: (missing,)})
        foo = self.entry(manifest, "Foo.cpp")

        self.assertEqual([missing], foo["mapping"]["stale_reviewed_destinations"])
        self.assertIn("stale_mapped_path", foo["blockers"])

    def test_reviewed_mapping_file_is_agent_editable_and_rejects_duplicates(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        path = fixture.root / provenance.REVIEWED_MAPPINGS_FILE
        path.write_text(
            json.dumps(
                {
                    "schema_version": 1,
                    "mappings": [
                        {
                            "source": "GeneralsMD/Code/GameEngine/Source/GameClient/Foo.cpp",
                            "destinations": [
                                "GeneralsRust/Code/GameEngine/GameClient/src/foo/mod.rs"
                            ],
                        }
                    ],
                }
            ),
            encoding="utf-8",
        )
        loaded = provenance.load_reviewed_mappings(fixture.root)
        self.assertEqual(1, len(loaded))
        manifest = provenance.build_manifest(fixture.root)
        self.assertEqual(1, manifest["reviewed_mapping_input"]["mapping_count"])
        self.assertEqual(64, len(manifest["reviewed_mapping_input"]["sha256"]))

        payload = json.loads(path.read_text(encoding="utf-8"))
        payload["mappings"].append(payload["mappings"][0])
        path.write_text(json.dumps(payload), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "duplicates reviewed source"):
            provenance.load_reviewed_mappings(fixture.root)

    def test_verify_generated_detects_and_clears_drift(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        output = fixture.root / "status"

        provenance.generate(fixture.root, output)
        self.assertEqual([], provenance.verify_generated(fixture.root, output))

        (output / "PORT_STATE.txt").write_text("stale\n", encoding="utf-8")
        self.assertEqual(
            ["PORT_STATE.txt"], provenance.verify_generated(fixture.root, output)
        )


if __name__ == "__main__":
    unittest.main()
