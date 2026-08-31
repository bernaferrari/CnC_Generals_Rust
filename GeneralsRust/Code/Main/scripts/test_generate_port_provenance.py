from __future__ import annotations

import hashlib
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


def file_sha(root: Path, relative: str) -> str:
    return hashlib.sha256((root / relative).read_bytes()).hexdigest()


def symbol_record(
    name: str,
    line: int,
    path: str,
    sha256: str,
    rust_symbols: tuple[str, ...],
    deviation: str | None = None,
) -> provenance.ReviewedSymbol:
    return provenance.ReviewedSymbol(
        name,
        line,
        (provenance.ReviewedAssignment(path, sha256, rust_symbols, deviation),),
    )


class ReviewedOwnershipSchemaTests(unittest.TestCase):
    def build_fixture(self) -> tuple[tempfile.TemporaryDirectory[str], ProvenanceFixture]:
        temporary = tempfile.TemporaryDirectory()
        fixture = ProvenanceFixture(Path(temporary.name))
        fixture.minimal_game_engine()
        fixture.write(
            "GeneralsRust/Code/GameEngine/GameClient/src/lib.rs",
            "pub mod foo;\npub mod bar;\npub mod qux;\npub mod split;\n",
        )
        fixture.write(
            "GeneralsRust/Code/GameEngine/GameClient/src/qux.rs",
            "pub fn update_int() {}\npub fn update_float() {}\n",
        )
        fixture.write(
            "GeneralsRust/Code/GameEngine/GameClient/src/split/mod.rs",
            "pub mod a;\npub mod b;\n",
        )
        fixture.write(
            "GeneralsRust/Code/GameEngine/GameClient/src/split/a.rs",
            "pub fn one() {}\npub fn two() {}\n",
        )
        fixture.write(
            "GeneralsRust/Code/GameEngine/GameClient/src/split/b.rs",
            "pub fn three() {}\n",
        )
        fixture.write(
            "GeneralsMD/Code/GameEngine/Source/GameClient/Qux.cpp",
            "void Qux::update(int) {}\nvoid Qux::update(float) {}\n",
        )
        fixture.write(
            "GeneralsMD/Code/GameEngine/Source/GameClient/Split.cpp",
            "void Split::one() {}\nvoid Split::two() {}\n"
            "void Split::three() {}\nvoid Split::four() {}\n",
        )
        return temporary, fixture

    FOO_CPP = "GeneralsMD/Code/GameEngine/Source/GameClient/Foo.cpp"
    QUX_CPP = "GeneralsMD/Code/GameEngine/Source/GameClient/Qux.cpp"
    SPLIT_CPP = "GeneralsMD/Code/GameEngine/Source/GameClient/Split.cpp"
    FOO_MOD = "GeneralsRust/Code/GameEngine/GameClient/src/foo/mod.rs"
    FOO_PART = "GeneralsRust/Code/GameEngine/GameClient/src/foo/part.rs"
    QUX_RS = "GeneralsRust/Code/GameEngine/GameClient/src/qux.rs"
    BAR_RS = "GeneralsRust/Code/GameEngine/GameClient/src/bar.rs"
    SPLIT_MOD = "GeneralsRust/Code/GameEngine/GameClient/src/split/mod.rs"
    SPLIT_A = "GeneralsRust/Code/GameEngine/GameClient/src/split/a.rs"
    SPLIT_B = "GeneralsRust/Code/GameEngine/GameClient/src/split/b.rs"

    @staticmethod
    def entry(manifest: dict[str, object], suffix: str) -> dict[str, object]:
        return next(
            entry
            for entry in manifest["entries"]
            if entry["source"]["path"].endswith(suffix)
        )

    def ownership(
        self,
        fixture: ProvenanceFixture,
        source: str,
        symbols: tuple[provenance.ReviewedSymbol, ...],
        ranges: tuple[provenance.ReviewedRange, ...] = (),
        source_sha256: str | None = None,
    ) -> provenance.ReviewedOwnership:
        return provenance.ReviewedOwnership(
            source,
            source_sha256 or file_sha(fixture.root, source),
            symbols,
            ranges,
        )

    def test_complete_valid_record_clears_both_ownership_blockers(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        ownership = self.ownership(
            fixture,
            self.FOO_CPP,
            (
                symbol_record(
                    "Foo::updateFoo",
                    1,
                    self.FOO_MOD,
                    file_sha(fixture.root, self.FOO_MOD),
                    ("update_foo",),
                ),
            ),
            (
                provenance.ReviewedRange(
                    1, 1, self.FOO_MOD, file_sha(fixture.root, self.FOO_MOD)
                ),
            ),
        )

        manifest = provenance.build_manifest(
            fixture.root,
            {self.FOO_CPP: (self.FOO_MOD,)},
            reviewed_ownership={self.FOO_CPP: ownership},
        )
        foo = self.entry(manifest, "Foo.cpp")

        self.assertEqual([], foo["blockers"])
        self.assertEqual("reviewed", foo["mapping"]["symbol_validation"])
        self.assertEqual("reviewed", foo["mapping"]["range_validation"])
        self.assertEqual([], foo["mapping"]["ownership_diagnostics"])
        destinations = {
            destination["path"]: destination
            for destination in foo["mapping"]["destinations"]
        }
        self.assertEqual(
            ["Foo::updateFoo"], destinations[self.FOO_MOD]["owned_symbols"]
        )
        self.assertEqual(
            [{"start_line": 1, "end_line": 1}],
            destinations[self.FOO_MOD]["owned_source_ranges"],
        )
        self.assertEqual([], destinations[self.FOO_PART]["owned_symbols"])
        self.assertEqual(1, manifest["summary"]["symbol_ownership_reviewed"])
        self.assertEqual(1, manifest["summary"]["source_range_ownership_reviewed"])
        self.assertEqual(
            "indexed",
            next(
                item["symbol_index_status"]
                for item in manifest["rust_file_index"]
                if item["path"] == self.FOO_MOD
            ),
        )

    def test_split_module_assignment_finds_symbols_in_fragments(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        a_sha = file_sha(fixture.root, self.SPLIT_A)
        b_sha = file_sha(fixture.root, self.SPLIT_B)
        mod_sha = file_sha(fixture.root, self.SPLIT_MOD)
        ownership = self.ownership(
            fixture,
            self.SPLIT_CPP,
            (
                symbol_record("Split::one", 1, self.SPLIT_MOD, mod_sha, ("one",)),
                symbol_record("Split::two", 2, self.SPLIT_A, a_sha, ("two",)),
                symbol_record("Split::three", 3, self.SPLIT_B, b_sha, ("three",)),
                symbol_record("Split::four", 4, self.SPLIT_B, b_sha, ("three",)),
            ),
            (
                provenance.ReviewedRange(1, 2, self.SPLIT_A, a_sha),
                provenance.ReviewedRange(3, 4, self.SPLIT_B, b_sha),
            ),
        )

        manifest = provenance.build_manifest(
            fixture.root,
            {self.SPLIT_CPP: (self.SPLIT_MOD,)},
            reviewed_ownership={self.SPLIT_CPP: ownership},
        )
        split = self.entry(manifest, "Split.cpp")

        self.assertEqual([], split["blockers"])
        self.assertEqual("reviewed", split["mapping"]["range_validation"])
        destinations = {
            destination["path"]: destination
            for destination in split["mapping"]["destinations"]
        }
        self.assertEqual(
            [{"start_line": 1, "end_line": 2}],
            destinations[self.SPLIT_A]["owned_source_ranges"],
        )
        self.assertEqual(
            [{"start_line": 3, "end_line": 4}],
            destinations[self.SPLIT_B]["owned_source_ranges"],
        )
        self.assertEqual(
            ["Split::one"], destinations[self.SPLIT_MOD]["owned_symbols"]
        )

    def test_overload_occurrences_must_all_be_claimed(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        qux_sha = file_sha(fixture.root, self.QUX_RS)
        partial = self.ownership(
            fixture,
            self.QUX_CPP,
            (symbol_record("Qux::update", 1, self.QUX_RS, qux_sha, ("update_int",)),),
        )
        complete = self.ownership(
            fixture,
            self.QUX_CPP,
            (
                symbol_record("Qux::update", 1, self.QUX_RS, qux_sha, ("update_int",)),
                symbol_record(
                    "Qux::update", 2, self.QUX_RS, qux_sha, ("update_float",)
                ),
            ),
        )

        partial_manifest = provenance.build_manifest(
            fixture.root, reviewed_ownership={self.QUX_CPP: partial}
        )
        qux = self.entry(partial_manifest, "Qux.cpp")
        self.assertIn("unreviewed_symbol_ownership", qux["blockers"])
        self.assertEqual(
            ["ownership_symbol:unassigned_cpp_symbol:Qux::update@2"],
            qux["mapping"]["ownership_diagnostics"],
        )
        self.assertEqual("invalid", qux["mapping"]["symbol_validation"])

        complete_manifest = provenance.build_manifest(
            fixture.root, reviewed_ownership={self.QUX_CPP: complete}
        )
        qux = self.entry(complete_manifest, "Qux.cpp")
        self.assertNotIn("unreviewed_symbol_ownership", qux["blockers"])
        self.assertEqual("reviewed", qux["mapping"]["symbol_validation"])
        self.assertEqual("not_required", qux["mapping"]["range_validation"])

    def test_stale_hashes_stay_red_with_exact_diagnostics(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        stale_source = self.ownership(
            fixture,
            self.QUX_CPP,
            (),
            source_sha256="0" * 64,
        )
        stale_destination = self.ownership(
            fixture,
            self.QUX_CPP,
            (symbol_record("Qux::update", 1, self.QUX_RS, "1" * 64, ("update_int",)),),
        )

        manifest = provenance.build_manifest(
            fixture.root, reviewed_ownership={self.QUX_CPP: stale_source}
        )
        qux = self.entry(manifest, "Qux.cpp")
        self.assertIn("unreviewed_symbol_ownership", qux["blockers"])
        self.assertIn(
            "ownership_symbol:stale_source_hash", qux["mapping"]["ownership_diagnostics"]
        )

        manifest = provenance.build_manifest(
            fixture.root, reviewed_ownership={self.QUX_CPP: stale_destination}
        )
        qux = self.entry(manifest, "Qux.cpp")
        self.assertIn(
            f"ownership_symbol:stale_destination_hash:{self.QUX_RS}",
            qux["mapping"]["ownership_diagnostics"],
        )
        self.assertIn("unreviewed_symbol_ownership", qux["blockers"])

    def test_range_gap_overlap_and_bounds_are_exact_diagnostics(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        a_sha = file_sha(fixture.root, self.SPLIT_A)
        b_sha = file_sha(fixture.root, self.SPLIT_B)

        def build(ranges: tuple[provenance.ReviewedRange, ...]) -> dict[str, object]:
            ownership = self.ownership(fixture, self.SPLIT_CPP, (), ranges)
            manifest = provenance.build_manifest(
                fixture.root,
                {self.SPLIT_CPP: (self.SPLIT_MOD,)},
                reviewed_ownership={self.SPLIT_CPP: ownership},
            )
            return self.entry(manifest, "Split.cpp")

        overlap = build(
            (
                provenance.ReviewedRange(1, 2, self.SPLIT_A, a_sha),
                provenance.ReviewedRange(2, 3, self.SPLIT_B, b_sha),
                provenance.ReviewedRange(4, 4, self.SPLIT_B, b_sha),
            )
        )
        self.assertIn(
            "ownership_range:overlap:2-3", overlap["mapping"]["ownership_diagnostics"]
        )

        gap = build(
            (
                provenance.ReviewedRange(1, 1, self.SPLIT_A, a_sha),
                provenance.ReviewedRange(3, 4, self.SPLIT_B, b_sha),
            )
        )
        self.assertIn(
            "ownership_range:gap:2-2", gap["mapping"]["ownership_diagnostics"]
        )

        out_of_bounds = build(
            (provenance.ReviewedRange(1, 99, self.SPLIT_A, a_sha),)
        )
        self.assertIn(
            "ownership_range:out_of_bounds:1-99",
            out_of_bounds["mapping"]["ownership_diagnostics"],
        )
        self.assertIn(
            "ownership_range:gap:1-4", out_of_bounds["mapping"]["ownership_diagnostics"]
        )

        unmapped = build(
            (provenance.ReviewedRange(1, 4, self.QUX_RS, a_sha),)
        )
        self.assertIn(
            f"ownership_range:unmapped_destination:{self.QUX_RS}",
            unmapped["mapping"]["ownership_diagnostics"],
        )

        for entry in (overlap, gap, out_of_bounds, unmapped):
            self.assertIn("unreviewed_source_range_ownership", entry["blockers"])
            self.assertIn("unreviewed_symbol_ownership", entry["blockers"])
            self.assertEqual("invalid", entry["mapping"]["range_validation"])

    def test_missing_symbols_and_bad_destinations_stay_red(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        qux_sha = file_sha(fixture.root, self.QUX_RS)
        bar_sha = file_sha(fixture.root, self.BAR_RS)
        ownership = self.ownership(
            fixture,
            self.QUX_CPP,
            (
                symbol_record("Qux::update", 9, self.QUX_RS, qux_sha, ("update_int",)),
                symbol_record("Qux::update", 1, self.QUX_RS, qux_sha, ("missing_fn",)),
                symbol_record(
                    "Qux::update", 2, self.QUX_RS, qux_sha, ("update_float",)
 ),
                symbol_record("Qux::update", 2, self.BAR_RS, bar_sha, ("emit",)),
                symbol_record(
                    "Qux::update",
                    2,
                    "GeneralsRust/Code/GameEngine/GameClient/src/gone.rs",
                    qux_sha,
                    ("update_float",),
                ),
            ),
        )

        manifest = provenance.build_manifest(
            fixture.root,
            {self.QUX_CPP: (self.QUX_RS, self.BAR_RS)},
            reviewed_ownership={self.QUX_CPP: ownership},
        )
        qux = self.entry(manifest, "Qux.cpp")
        diagnostics = qux["mapping"]["ownership_diagnostics"]

        self.assertIn("ownership_symbol:unknown_cpp_symbol:Qux::update@9", diagnostics)
        self.assertIn(
            f"ownership_symbol:missing_rust_symbol:missing_fn@{self.QUX_RS}",
            diagnostics,
        )
        self.assertIn(
            f"ownership_symbol:non_implementation_destination:{self.BAR_RS}",
            diagnostics,
        )
        self.assertIn(
            "ownership_symbol:stale_destination_path:"
            "GeneralsRust/Code/GameEngine/GameClient/src/gone.rs",
            diagnostics,
        )
        self.assertNotIn("ownership_symbol:unassigned_cpp_symbol:Qux::update@2", diagnostics)
        self.assertIn("unreviewed_symbol_ownership", qux["blockers"])
        self.assertEqual("invalid", qux["mapping"]["symbol_validation"])

    def test_representation_safe_deviation_is_credited_only_when_known(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        qux_sha = file_sha(fixture.root, self.QUX_RS)
        allowed = self.ownership(
            fixture,
            self.QUX_CPP,
            (
                symbol_record(
                    "Qux::update",
                    1,
                    self.QUX_RS,
                    qux_sha,
                    ("update_int",),
                    "representation_safe_enum_cleanup",
                ),
                symbol_record(
                    "Qux::update", 2, self.QUX_RS, qux_sha, ("update_float",)
                ),
            ),
        )
        manifest = provenance.build_manifest(
            fixture.root, reviewed_ownership={self.QUX_CPP: allowed}
        )
        qux = self.entry(manifest, "Qux.cpp")
        self.assertNotIn("unreviewed_symbol_ownership", qux["blockers"])
        self.assertIn("unreviewed_mapping", qux["blockers"])
        self.assertEqual(
            ["representation_safe_enum_cleanup"], qux["allowed_deviations"]
        )

        unknown = self.ownership(
            fixture,
            self.QUX_CPP,
            (
                symbol_record(
                    "Qux::update", 1, self.QUX_RS, qux_sha, ("update_int",), "bogus"
                ),
                symbol_record(
                    "Qux::update", 2, self.QUX_RS, qux_sha, ("update_float",)
                ),
            ),
        )
        manifest = provenance.build_manifest(
            fixture.root, reviewed_ownership={self.QUX_CPP: unknown}
        )
        qux = self.entry(manifest, "Qux.cpp")
        self.assertIn(
            "ownership_symbol:unknown_deviation:bogus",
            qux["mapping"]["ownership_diagnostics"],
        )
        self.assertIn("unreviewed_symbol_ownership", qux["blockers"])
        self.assertEqual([], qux["allowed_deviations"])

    def test_orphan_ownership_records_are_reported_not_hidden(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        orphan = self.ownership(
            fixture,
            "GeneralsMD/Code/GameEngine/Source/GameClient/Ghost.cpp",
            (),
            source_sha256="d" * 64,
        )
        network = self.ownership(
            fixture,
            "GeneralsMD/Code/GameEngine/Source/GameNetwork/Net.cpp",
            (),
        )

        manifest = provenance.build_manifest(
            fixture.root,
            reviewed_ownership={
                "GeneralsMD/Code/GameEngine/Source/GameClient/Ghost.cpp": orphan,
                "GeneralsMD/Code/GameEngine/Source/GameNetwork/Net.cpp": network,
            },
        )

        self.assertEqual(2, manifest["summary"]["orphan_ownership_records"])
        self.assertEqual(
            [
                "GeneralsMD/Code/GameEngine/Source/GameClient/Ghost.cpp",
                "GeneralsMD/Code/GameEngine/Source/GameNetwork/Net.cpp",
            ],
            manifest["ownership_review"]["orphan_sources"],
        )
        self.assertEqual(2, manifest["ownership_review"]["record_count"])

    def test_ownership_edits_change_the_input_digest(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        qux_sha = file_sha(fixture.root, self.QUX_RS)
        ownership = self.ownership(
            fixture,
            self.QUX_CPP,
            (symbol_record("Qux::update", 1, self.QUX_RS, qux_sha, ("update_int",)),),
        )

        without = provenance.build_manifest(fixture.root, reviewed_ownership={})
        with_record = provenance.build_manifest(
            fixture.root, reviewed_ownership={self.QUX_CPP: ownership}
        )

        self.assertNotEqual(without["input_digest"], with_record["input_digest"])

    def test_load_reviewed_ownership_rejects_malformed_records(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        path = fixture.root / provenance.REVIEWED_MAPPINGS_FILE
        valid_symbol = {
            "name": "Foo::updateFoo",
            "line": 1,
            "assignments": [
                {
                    "path": self.FOO_MOD,
                    "sha256": "a" * 64,
                    "symbols": ["update_foo"],
                }
            ],
        }
        valid_record = {
            "source": self.FOO_CPP,
            "source_sha256": "b" * 64,
            "symbols": [valid_symbol],
            "source_ranges": [
                {"start_line": 1, "end_line": 1, "path": self.FOO_MOD, "sha256": "c" * 64}
            ],
        }

        def write_payload(record: object) -> None:
            path.write_text(
                json.dumps(
                    {"schema_version": 1, "mappings": [], "ownership": [record]}
                ),
                encoding="utf-8",
            )

        write_payload(valid_record)
        loaded = provenance.load_reviewed_ownership(fixture.root)
        self.assertEqual(
            (self.FOO_MOD, "a" * 64),
            (loaded[self.FOO_CPP].symbols[0].assignments[0].path,
             loaded[self.FOO_CPP].symbols[0].assignments[0].sha256),
        )

        malformed = [
            ("bad hash", {**valid_record, "source_sha256": "zz"}),
            (
                "duplicate symbol",
                {**valid_record, "symbols": [valid_symbol, valid_symbol]},
            ),
            (
                "empty assignments",
                {
                    **valid_record,
                    "symbols": [
                        {"name": "Foo::updateFoo", "line": 1, "assignments": []}
                    ],
                },
            ),
            (
                "range end before start",
                {
                    **valid_record,
                    "source_ranges": [
                        {
                            "start_line": 9,
                            "end_line": 2,
                            "path": self.FOO_MOD,
                            "sha256": "c" * 64,
                        }
                    ],
                },
            ),
        ]
        for label, record in malformed:
            with self.subTest(label=label):
                write_payload(record)
                with self.assertRaises(ValueError):
                    provenance.load_reviewed_ownership(fixture.root)

        path.write_text(
            json.dumps({"schema_version": 1, "mappings": [], "ownership": []}),
            encoding="utf-8",
        )
        self.assertEqual({}, provenance.load_reviewed_ownership(fixture.root))


if __name__ == "__main__":
    unittest.main()
