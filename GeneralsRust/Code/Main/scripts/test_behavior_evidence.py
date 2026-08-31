from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import behavior_evidence  # noqa: E402
import generate_port_provenance as provenance  # noqa: E402
import port_dashboard  # noqa: E402


FOO_CPP = "GeneralsMD/Code/GameEngine/Source/GameClient/Foo.cpp"
FOO_HEADER = "GeneralsMD/Code/GameEngine/Include/GameClient/Foo.h"
FOO_MOD = "GeneralsRust/Code/GameEngine/GameClient/src/foo/mod.rs"
FOO_PART = "GeneralsRust/Code/GameEngine/GameClient/src/foo/part.rs"
FOO_TEST = "GeneralsRust/Code/GameEngine/GameClient/src/foo_test.rs"
BAR_CPP = "GeneralsMD/Code/GameEngine/Source/GameClient/Bar.cpp"
BAR_RS = "GeneralsRust/Code/GameEngine/GameClient/src/bar.rs"
SCENARIO = "parity_scenarios/foo_smoke.v1.json"
ARTIFACT = "parity_golden/foo_smoke_cpp_trace.v1.json"


class EvidenceFixture:
    def __init__(self, root: Path) -> None:
        self.root = root

    def write(self, relative: str, content: str) -> Path:
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
        return path

    def sha(self, relative: str) -> str:
        return behavior_evidence.sha256_file(self.root / relative)

    def pin(self, relative: str) -> dict[str, str]:
        return {"path": relative, "sha256": self.sha(relative)}

    def minimal_units(self) -> None:
        self.write(
            "GeneralsRust/Code/GameEngine/GameClient/Cargo.toml",
            '[package]\nname = "fixture-client"\nversion = "0.1.0"\n',
        )
        self.write(
            "GeneralsRust/Code/GameEngine/GameClient/src/lib.rs",
            "pub mod foo;\npub mod bar;\n#[cfg(test)]\nmod foo_test;\n",
        )
        self.write(
            FOO_MOD,
            'include!("part.rs");\npub fn update_foo() {}\n',
        )
        self.write(FOO_PART, "pub struct FooState;\n")
        self.write(FOO_TEST, "pub fn fake_foo_parity_test() {}\n")
        self.write(BAR_RS, "pub fn update_bar() {}\n")
        self.write(FOO_CPP, "void Foo::updateFoo() {}\n")
        self.write(BAR_CPP, "void Bar::update() {}\n")
        self.write(FOO_HEADER, "class Foo {};\n")
        self.write(SCENARIO, '{"scenario": "foo_smoke", "version": 1}\n')
        self.write(ARTIFACT, '{"frames": [1, 2, 3]}\n')

    def promoting_record(
        self,
        kind: str,
        *,
        scope: str | None = None,
        scenario: str = SCENARIO,
        artifacts: list[str] | None = None,
        commands: list[tuple[str, ...]] | None = None,
        exit_code: int = 0,
        deviation: str | None = None,
    ) -> dict[str, object]:
        payload: dict[str, object] = {
            "kind": kind,
            "scope": scope or behavior_evidence.REQUIRED_SCOPES_BY_KIND[kind][0],
            "scenario": self.pin(scenario),
            "artifacts": [
                {"role": "expected", **self.pin(path)}
                for path in (artifacts if artifacts is not None else [ARTIFACT])
            ],
            "commands": [
                {"argv": list(argv), "exit_code": exit_code}
                for argv in (commands or [("python3", "run_differential.py")])
            ],
        }
        if kind in behavior_evidence.DEVIATIONS_BY_KIND:
            payload["deviation"] = (
                deviation
                if deviation is not None
                else behavior_evidence.DEVIATIONS_BY_KIND[kind]
            )
        return payload

    def evidence_unit(
        self,
        source: str,
        destinations: list[str],
        records: list[dict[str, object]],
    ) -> dict[str, object]:
        return {
            "source": source,
            "source_sha256": self.sha(source),
            "destinations": [self.pin(path) for path in destinations],
            "records": records,
        }

    def write_evidence(self, units: list[dict[str, object]]) -> None:
        self.root.joinpath(behavior_evidence.EVIDENCE_FILE).write_text(
            json.dumps({"schema_version": 1, "units": units}), encoding="utf-8"
        )

    def write_reviewed(self, mappings: dict[str, list[str]]) -> None:
        self.root.joinpath(provenance.REVIEWED_MAPPINGS_FILE).write_text(
            json.dumps(
                {
                    "schema_version": 1,
                    "mappings": [
                        {"source": source, "destinations": destinations}
                        for source, destinations in sorted(mappings.items())
                    ],
                }
            ),
            encoding="utf-8",
        )


class BehaviorEvidenceTests(unittest.TestCase):
    def build_fixture(self) -> tuple[tempfile.TemporaryDirectory[str], EvidenceFixture]:
        temporary = tempfile.TemporaryDirectory()
        fixture = EvidenceFixture(Path(temporary.name))
        fixture.minimal_units()
        return temporary, fixture

    @staticmethod
    def entry(manifest: dict[str, object], suffix: str) -> dict[str, object]:
        return next(
            entry
            for entry in manifest["entries"]
            if entry["source"]["path"].endswith(suffix)
        )

    def default_reviewed(self) -> dict[str, list[str]]:
        return {
            FOO_CPP: [FOO_MOD],
            BAR_CPP: [BAR_RS],
            FOO_HEADER: [FOO_MOD],
        }

    # ------------------------------------------------------------------
    # Loader: schema shape
    # ------------------------------------------------------------------

    def test_loader_accepts_every_evidence_kind_without_conflating_levels(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)

        fixture.write_evidence(
            [
                fixture.evidence_unit(
                    FOO_CPP,
                    [FOO_MOD, FOO_PART],
                    [
                        fixture.promoting_record("original_cpp_differential"),
                        fixture.promoting_record("golden_retail"),
                        fixture.promoting_record("snapshot_bytes"),
                        fixture.promoting_record("deterministic_timing_rng"),
                        fixture.promoting_record("rendering_wgpu_tolerance"),
                        fixture.promoting_record("enum_representation"),
                        {"kind": "prose", "note": "Manual notes can never promote."},
                        {"kind": "rust_vs_rust", "note": "Fixture-only comparison."},
                    ],
                )
            ]
        )

        units = behavior_evidence.load_behavior_evidence(fixture.root)

        self.assertEqual([FOO_CPP], list(units))
        records = units[FOO_CPP].records
        self.assertEqual(
            sorted(
                [
                    "original_cpp_differential",
                    "golden_retail",
                    "snapshot_bytes",
                    "deterministic_timing_rng",
                    "rendering_wgpu_tolerance",
                    "enum_representation",
                    "prose",
                    "rust_vs_rust",
                ]
            ),
            sorted(record.kind for record in records),
        )
        self.assertEqual(
            {"original_cpp", "retail"},
            {record.scope for record in records if record.kind in behavior_evidence.PROMOTING_KINDS},
        )

    def test_loader_rejects_malformed_or_forged_structures(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)

        def base_unit() -> dict[str, object]:
            return fixture.evidence_unit(
                FOO_CPP,
                [FOO_MOD, FOO_PART],
                [fixture.promoting_record("original_cpp_differential")],
            )

        def base_document() -> dict[str, object]:
            return {"schema_version": 1, "units": [base_unit()]}

        def with_schema_version(document: dict[str, object]) -> None:
            document["schema_version"] = 2

        def with_duplicate_source(document: dict[str, object]) -> None:
            document["units"].append(base_unit())

        def with_bad_source_hash(document: dict[str, object]) -> None:
            document["units"][0]["source_sha256"] = "deadbeef"

        def with_empty_destinations(document: dict[str, object]) -> None:
            document["units"][0]["destinations"] = []

        def with_empty_records(document: dict[str, object]) -> None:
            document["units"][0]["records"] = []

        def with_unknown_kind(document: dict[str, object]) -> None:
            document["units"][0]["records"][0]["kind"] = "gut_feeling"

        def with_missing_commands(document: dict[str, object]) -> None:
            del document["units"][0]["records"][0]["commands"]

        def with_extra_field(document: dict[str, object]) -> None:
            document["units"][0]["records"][0]["notes"] = "trust me"

        def with_wrong_differential_scope(document: dict[str, object]) -> None:
            document["units"][0]["records"][0]["scope"] = "retail"

        def with_wrong_retail_scope(document: dict[str, object]) -> None:
            document["units"][0]["records"][0] = fixture.promoting_record(
                "golden_retail", scope="original_cpp"
            )

        def with_wrong_deviation(document: dict[str, object]) -> None:
            document["units"][0]["records"][0] = fixture.promoting_record(
                "rendering_wgpu_tolerance", deviation="representation_safe_enum_cleanup"
            )

        def with_boolean_exit_code(document: dict[str, object]) -> None:
            document["units"][0]["records"][0]["commands"][0]["exit_code"] = True

        def with_empty_artifacts(document: dict[str, object]) -> None:
            document["units"][0]["records"][0]["artifacts"] = []

        def with_commandless_record(document: dict[str, object]) -> None:
            document["units"][0]["records"][0]["commands"] = []

        def with_incomplete_scenario(document: dict[str, object]) -> None:
            document["units"][0]["records"][0]["scenario"] = {"path": SCENARIO}

        def with_prose_scope(document: dict[str, object]) -> None:
            document["units"][0]["records"][0] = {
                "kind": "prose",
                "scope": "original_cpp",
                "note": "scoped prose is still rejected",
            }

        def with_empty_note(document: dict[str, object]) -> None:
            document["units"][0]["records"][0] = {"kind": "prose", "note": "  "}

        def with_extra_pin_field(document: dict[str, object]) -> None:
            document["units"][0]["destinations"][0]["role"] = "implementation"

        cases = [
            ("schema_version", with_schema_version, r"schema_version must be 1"),
            ("duplicate_source", with_duplicate_source, r"duplicates evidence source"),
            ("bad_source_hash", with_bad_source_hash, r"64-hex sha256"),
            ("empty_destinations", with_empty_destinations, r"destinations must be a non-empty list"),
            ("empty_records", with_empty_records, r"records must be a non-empty list"),
            ("unknown_kind", with_unknown_kind, r"unknown evidence kind"),
            ("missing_commands", with_missing_commands, r"must have exactly"),
            ("extra_field", with_extra_field, r"must have exactly"),
            ("wrong_differential_scope", with_wrong_differential_scope, r"scope must be one of"),
            ("wrong_retail_scope", with_wrong_retail_scope, r"scope must be one of"),
            ("wrong_deviation", with_wrong_deviation, r"must declare the deviation"),
            ("boolean_exit_code", with_boolean_exit_code, r"exit_code must be an integer"),
            ("empty_artifacts", with_empty_artifacts, r"artifacts must be a non-empty list"),
            ("commandless_record", with_commandless_record, r"commands must be a non-empty list"),
            ("incomplete_scenario", with_incomplete_scenario, r"exactly path and sha256"),
            ("prose_scope", with_prose_scope, r"take exactly kind and note"),
            ("empty_note", with_empty_note, r"needs a non-empty string"),
            ("extra_pin_field", with_extra_pin_field, r"exactly path and sha256"),
        ]
        for name, mutate, pattern in cases:
            with self.subTest(case=name):
                fixture.write_evidence([base_unit()])
                document = base_document()
                mutate(document)
                fixture.root.joinpath(behavior_evidence.EVIDENCE_FILE).write_text(
                    json.dumps(document), encoding="utf-8"
                )
                with self.assertRaisesRegex(ValueError, pattern):
                    behavior_evidence.load_behavior_evidence(fixture.root)

    def test_loader_rejects_escaping_paths_and_noop_commands(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)

        def document() -> dict[str, object]:
            return {
                "schema_version": 1,
                "units": [
                    fixture.evidence_unit(
                        FOO_CPP,
                        [FOO_MOD, FOO_PART],
                        [fixture.promoting_record("original_cpp_differential")],
                    )
                ],
            }

        cases = [
            (
                "source_parent_traversal",
                lambda raw: raw["units"][0].update(source="../Foo.cpp"),
                r"repository-relative path",
            ),
            (
                "absolute_artifact",
                lambda raw: raw["units"][0]["records"][0]["artifacts"][0].update(
                    path="/tmp/forged-output.json"
                ),
                r"repository-relative path",
            ),
            (
                "windows_destination",
                lambda raw: raw["units"][0]["destinations"][0].update(
                    path=r"C:\\forged.rs"
                ),
                r"repository-relative path",
            ),
            (
                "noop_true",
                lambda raw: raw["units"][0]["records"][0].update(
                    commands=[{"argv": ["true", "pretend-replay"], "exit_code": 0}]
                ),
                r"not a replay command",
            ),
            (
                "shell_wrapper",
                lambda raw: raw["units"][0]["records"][0].update(
                    commands=[{"argv": ["sh", "run-replay.sh"], "exit_code": 0}]
                ),
                r"shell wrappers are not replay commands",
            ),
        ]
        for name, mutate, pattern in cases:
            with self.subTest(case=name):
                raw = document()
                mutate(raw)
                fixture.root.joinpath(behavior_evidence.EVIDENCE_FILE).write_text(
                    json.dumps(raw), encoding="utf-8"
                )
                with self.assertRaisesRegex(ValueError, pattern):
                    behavior_evidence.load_behavior_evidence(fixture.root)

    # ------------------------------------------------------------------
    # Promotion: valid evidence
    # ------------------------------------------------------------------

    def test_valid_original_cpp_differential_promotes_reviewed_unit(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        fixture.write_reviewed(self.default_reviewed())
        fixture.write_evidence(
            [
                fixture.evidence_unit(
                    FOO_CPP,
                    [FOO_MOD, FOO_PART],
                    [fixture.promoting_record("original_cpp_differential")],
                )
            ]
        )

        manifest = provenance.build_manifest(fixture.root)
        foo = self.entry(manifest, "Foo.cpp")

        self.assertEqual("verified", foo["behavior"]["status"])
        self.assertEqual("exact", foo["behavior"]["confidence"])
        self.assertEqual(["original_cpp"], foo["behavior"]["scopes"])
        self.assertEqual(1, manifest["summary"]["behavior_verified"])
        evidence_summary = manifest["summary"]["behavior_evidence"]
        self.assertEqual(1, evidence_summary["verified_units"])
        self.assertEqual({"original_cpp": 1}, evidence_summary["by_evidence_scope"])
        self.assertEqual({"exact": 1, "tolerance": 0}, evidence_summary["by_confidence"])
        self.assertEqual(1, manifest["behavior_evidence_input"]["unit_count"])
        self.assertEqual(
            64, len(manifest["behavior_evidence_input"]["sha256"])
        )
        record = foo["behavior"]["evidence"][0]
        self.assertTrue(record["promoting"])
        self.assertEqual("exact", record["level"])
        self.assertEqual(
            ["python3", "run_differential.py"], record["commands"][0]["argv"]
        )

    def test_golden_retail_evidence_pins_retail_scope(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        fixture.write_reviewed(self.default_reviewed())
        fixture.write_evidence(
            [
                fixture.evidence_unit(
                    BAR_CPP, [BAR_RS], [fixture.promoting_record("golden_retail")]
                )
            ]
        )

        manifest = provenance.build_manifest(fixture.root)
        bar = self.entry(manifest, "Bar.cpp")

        self.assertEqual("verified", bar["behavior"]["status"])
        self.assertEqual(["retail"], bar["behavior"]["scopes"])
        self.assertEqual({"retail": 1}, manifest["summary"]["behavior_evidence"]["by_evidence_scope"])

    def test_tolerance_kinds_promote_only_at_tolerance_confidence(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        fixture.write_reviewed(self.default_reviewed())
        fixture.write_evidence(
            [
                fixture.evidence_unit(
                    BAR_CPP,
                    [BAR_RS],
                    [
                        fixture.promoting_record("rendering_wgpu_tolerance"),
                        fixture.promoting_record("enum_representation"),
                    ],
                )
            ]
        )

        manifest = provenance.build_manifest(fixture.root)
        bar = self.entry(manifest, "Bar.cpp")

        self.assertEqual("verified", bar["behavior"]["status"])
        self.assertEqual("tolerance", bar["behavior"]["confidence"])
        deviations = {
            entry["kind"]: entry["deviation"] for entry in bar["behavior"]["evidence"]
        }
        self.assertEqual("directx_to_wgpu", deviations["rendering_wgpu_tolerance"])
        self.assertEqual(
            "representation_safe_enum_cleanup", deviations["enum_representation"]
        )
        self.assertEqual(
            {"exact": 0, "tolerance": 1},
            manifest["summary"]["behavior_evidence"]["by_confidence"],
        )

    # ------------------------------------------------------------------
    # Stale, partial, and forged evidence
    # ------------------------------------------------------------------

    def stale_case(self, mutate_fixture) -> tuple[dict[str, object], dict[str, object], list[str]]:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        fixture.write_reviewed(self.default_reviewed())
        fixture.write_evidence(
            [
                fixture.evidence_unit(
                    FOO_CPP, [FOO_MOD, FOO_PART],
                    [fixture.promoting_record("original_cpp_differential")],
                )
            ]
        )
        mutate_fixture(fixture)
        manifest = provenance.build_manifest(fixture.root)
        foo = self.entry(manifest, "Foo.cpp")
        reasons = [
            reason
            for rejection in foo["behavior"]["rejected_evidence"]
            for reason in (
                rejection["reasons"]
                if "reasons" in rejection
                else [rejection["reason"]]
            )
        ]
        return manifest, foo, reasons

    def test_stale_source_hash_cannot_promote(self) -> None:
        manifest, foo, reasons = self.stale_case(
            lambda fixture: fixture.write(
                FOO_CPP, "void Foo::updateFoo() { /* changed */ }\n"
            )
        )

        self.assertEqual("not_verified", foo["behavior"]["status"])
        self.assertIn("stale_source_hash", reasons)
        self.assertEqual(0, manifest["summary"]["behavior_verified"])

    def test_stale_destination_hash_cannot_promote(self) -> None:
        manifest, foo, reasons = self.stale_case(
            lambda fixture: fixture.write(FOO_PART, "pub struct FooState2;\n")
        )

        self.assertEqual("not_verified", foo["behavior"]["status"])
        self.assertIn(f"stale_destination_hash:{FOO_PART}", reasons)
        self.assertEqual(0, manifest["summary"]["behavior_verified"])

    def test_partial_destination_coverage_cannot_promote(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        fixture.write_reviewed(self.default_reviewed())
        fixture.write_evidence(
            [
                fixture.evidence_unit(
                    FOO_CPP,
                    [FOO_MOD],
                    [fixture.promoting_record("original_cpp_differential")],
                )
            ]
        )

        manifest = provenance.build_manifest(fixture.root)
        foo = self.entry(manifest, "Foo.cpp")

        self.assertEqual("not_verified", foo["behavior"]["status"])
        reasons = [
            rejection["reason"]
            for rejection in foo["behavior"]["rejected_evidence"]
            if "reason" in rejection
        ]
        self.assertIn(f"uncovered_destination:{FOO_PART}", reasons)

    def test_forged_artifact_hash_cannot_promote(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        fixture.write_reviewed(self.default_reviewed())
        record = fixture.promoting_record("original_cpp_differential")
        record["artifacts"][0]["sha256"] = fixture.sha(FOO_CPP)
        fixture.write_evidence(
            [fixture.evidence_unit(FOO_CPP, [FOO_MOD, FOO_PART], [record])]
        )

        manifest = provenance.build_manifest(fixture.root)
        foo = self.entry(manifest, "Foo.cpp")

        self.assertEqual("not_verified", foo["behavior"]["status"])
        reasons = [
            reason
            for rejection in foo["behavior"]["rejected_evidence"]
            for reason in rejection.get("reasons", [])
        ]
        self.assertIn(f"stale_artifact_hash:{ARTIFACT}", reasons)

    def test_forged_scenario_hash_cannot_promote(self) -> None:
        manifest, foo, reasons = self.stale_case(
            lambda fixture: fixture.write(SCENARIO, '{"scenario": "edited"}\n')
        )

        self.assertEqual("not_verified", foo["behavior"]["status"])
        self.assertIn(f"stale_scenario_hash:{SCENARIO}", reasons)

    def test_failed_command_cannot_promote(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        fixture.write_reviewed(self.default_reviewed())
        fixture.write_evidence(
            [
                fixture.evidence_unit(
                    FOO_CPP,
                    [FOO_MOD, FOO_PART],
                    [
                        fixture.promoting_record(
                            "original_cpp_differential", exit_code=1
                        )
                    ],
                )
            ]
        )

        manifest = provenance.build_manifest(fixture.root)
        foo = self.entry(manifest, "Foo.cpp")

        self.assertEqual("not_verified", foo["behavior"]["status"])
        reasons = [
            reason
            for rejection in foo["behavior"]["rejected_evidence"]
            for reason in rejection.get("reasons", [])
        ]
        self.assertIn("failed_command:python3 run_differential.py", reasons)
        self.assertEqual(0, manifest["summary"]["behavior_verified"])

    def test_test_only_mapping_cannot_promote(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        fixture.write_reviewed({FOO_CPP: [FOO_TEST]})
        fixture.write_evidence(
            [
                fixture.evidence_unit(
                    FOO_CPP,
                    [FOO_TEST],
                    [fixture.promoting_record("original_cpp_differential")],
                )
            ]
        )

        manifest = provenance.build_manifest(fixture.root)
        foo = self.entry(manifest, "Foo.cpp")
        reasons = [
            rejection["reason"]
            for rejection in foo["behavior"]["rejected_evidence"]
            if "reason" in rejection
        ]

        self.assertEqual("not_verified", foo["behavior"]["status"])
        self.assertFalse(foo["mapping"]["reviewed_path_implementation"])
        self.assertIn(f"non_implementation_destination:{FOO_TEST}", reasons)
        self.assertIn("no_reachable_implementation_destination", reasons)

    def test_input_only_or_unknown_artifacts_cannot_promote(self) -> None:
        for role, expected_reason in (
            ("input", "missing_output_artifact"),
            ("trust_me", "unknown_artifact_role:trust_me"),
        ):
            with self.subTest(role=role):
                temporary, fixture = self.build_fixture()
                self.addCleanup(temporary.cleanup)
                fixture.write_reviewed(self.default_reviewed())
                record = fixture.promoting_record("original_cpp_differential")
                record["artifacts"][0]["role"] = role
                fixture.write_evidence(
                    [fixture.evidence_unit(FOO_CPP, [FOO_MOD, FOO_PART], [record])]
                )

                manifest = provenance.build_manifest(fixture.root)
                foo = self.entry(manifest, "Foo.cpp")
                reasons = [
                    reason
                    for rejection in foo["behavior"]["rejected_evidence"]
                    for reason in rejection.get("reasons", [])
                ]

                self.assertEqual("not_verified", foo["behavior"]["status"])
                self.assertIn(expected_reason, reasons)

    def test_symlink_escape_cannot_promote(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        outside = tempfile.TemporaryDirectory()
        self.addCleanup(outside.cleanup)
        outside_root = Path(outside.name)
        outside_artifact = outside_root / "forged-output.json"
        outside_artifact.write_text('{"frames": [1, 2, 3]}\n', encoding="utf-8")
        fixture.root.joinpath("escape").symlink_to(outside_root, target_is_directory=True)

        fixture.write_reviewed(self.default_reviewed())
        escaped_path = "escape/forged-output.json"
        record = fixture.promoting_record(
            "original_cpp_differential", artifacts=[escaped_path]
        )
        fixture.write_evidence(
            [fixture.evidence_unit(FOO_CPP, [FOO_MOD, FOO_PART], [record])]
        )

        manifest = provenance.build_manifest(fixture.root)
        foo = self.entry(manifest, "Foo.cpp")
        reasons = [
            reason
            for rejection in foo["behavior"]["rejected_evidence"]
            for reason in rejection.get("reasons", [])
        ]

        self.assertEqual("not_verified", foo["behavior"]["status"])
        self.assertIn(f"path_outside_repo:{escaped_path}", reasons)

    # ------------------------------------------------------------------
    # Non-promoting evidence classes
    # ------------------------------------------------------------------

    def test_prose_and_rust_vs_rust_records_never_promote(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        fixture.write_reviewed(self.default_reviewed())
        fixture.write_evidence(
            [
                fixture.evidence_unit(
                    FOO_CPP,
                    [FOO_MOD, FOO_PART],
                    [
                        {"kind": "prose", "note": "Looks equivalent to me."},
                        {"kind": "rust_vs_rust", "note": "Rust fixture trace matched."},
                    ],
                )
            ]
        )

        manifest = provenance.build_manifest(fixture.root)
        foo = self.entry(manifest, "Foo.cpp")

        self.assertEqual("not_verified", foo["behavior"]["status"])
        self.assertEqual([], foo["behavior"]["rejected_evidence"])
        self.assertEqual(
            {"prose", "rust_vs_rust"},
            {entry["kind"] for entry in foo["behavior"]["evidence"]},
        )
        self.assertTrue(
            all(entry["promoting"] is False for entry in foo["behavior"]["evidence"])
        )
        self.assertEqual(0, manifest["summary"]["behavior_verified"])
        self.assertEqual(
            2, manifest["summary"]["behavior_evidence"]["informational_records"]
        )

    def test_path_only_mapping_and_unreviewed_units_cannot_promote(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        # No reviewed mapping file: evidence alone must not create path credit.
        fixture.write_evidence(
            [
                fixture.evidence_unit(
                    FOO_CPP, [FOO_MOD, FOO_PART],
                    [fixture.promoting_record("original_cpp_differential")],
                )
            ]
        )

        manifest = provenance.build_manifest(fixture.root)
        foo = self.entry(manifest, "Foo.cpp")

        self.assertEqual("not_verified", foo["behavior"]["status"])
        reasons = [
            rejection["reason"]
            for rejection in foo["behavior"]["rejected_evidence"]
            if "reason" in rejection
        ]
        self.assertIn("unreviewed_mapping", reasons)
        self.assertFalse(foo["mapping"]["reviewed_path_implementation"])

    def test_headers_and_deferred_units_cannot_promote(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        fixture.write_reviewed(self.default_reviewed())
        fixture.write_evidence(
            [
                fixture.evidence_unit(
                    FOO_HEADER, [FOO_MOD, FOO_PART],
                    [fixture.promoting_record("original_cpp_differential")],
                )
            ]
        )

        manifest = provenance.build_manifest(fixture.root)
        header = self.entry(manifest, "Foo.h")

        self.assertEqual("header", header["unit_kind"])
        self.assertEqual("not_verified", header["behavior"]["status"])
        reasons = [
            rejection["reason"]
            for rejection in header["behavior"]["rejected_evidence"]
            if "reason" in rejection
        ]
        self.assertIn("unit_kind_not_promotable", reasons)

    def test_unmatched_evidence_sources_are_reported(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        fixture.write_reviewed(self.default_reviewed())
        typo_unit = fixture.evidence_unit(
            FOO_CPP, [FOO_MOD, FOO_PART],
            [fixture.promoting_record("original_cpp_differential")],
        )
        typo_unit["source"] = "GeneralsMD/Code/GameEngine/Source/GameClient/Fooo.cpp"
        fixture.write_evidence([typo_unit])

        manifest = provenance.build_manifest(fixture.root)

        self.assertEqual(
            [typo_unit["source"]],
            manifest["summary"]["behavior_evidence"]["unmatched_units"],
        )
        self.assertEqual(0, manifest["summary"]["behavior_verified"])

    # ------------------------------------------------------------------
    # Generator integration and dashboard reporting
    # ------------------------------------------------------------------

    def test_manifest_with_evidence_is_deterministic_and_drift_detected(self) -> None:
        temporary, fixture = self.build_fixture()
        self.addCleanup(temporary.cleanup)
        fixture.write_reviewed(self.default_reviewed())
        fixture.write_evidence(
            [
                fixture.evidence_unit(
                    FOO_CPP, [FOO_MOD, FOO_PART],
                    [fixture.promoting_record("original_cpp_differential")],
                )
            ]
        )
        output_a = fixture.root / "out-a"
        output_b = fixture.root / "out-b"

        first = provenance.generate(fixture.root, output_a)
        second = provenance.generate(fixture.root, output_b)

        self.assertEqual(first, second)
        self.assertEqual(
            (output_a / "PORT_PROVENANCE_MANIFEST.json").read_bytes(),
            (output_b / "PORT_PROVENANCE_MANIFEST.json").read_bytes(),
        )
        self.assertEqual([], provenance.verify_generated(fixture.root, output_a))

        tampered = json.loads(
            (output_a / "PORT_PROVENANCE_MANIFEST.json").read_text(encoding="utf-8")
        )
        tampered["entries"][0]["behavior"]["status"] = "verified"
        (output_a / "PORT_PROVENANCE_MANIFEST.json").write_text(
            json.dumps(tampered, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        self.assertEqual(
            ["PORT_PROVENANCE_MANIFEST.json"],
            provenance.verify_generated(fixture.root, output_a),
        )

    def test_dashboard_ladder_shows_evidence_scope_and_confidence(self) -> None:
        ladder = port_dashboard.confidence_ladder(
            {
                "required_translation_units": 10,
                "reachable_implementation_candidates": 5,
                "reviewed_path_implementations": 2,
                "behavior_verified": 1,
                "blockers_by_kind": {},
                "behavior_evidence": {
                    "verified_units": 1,
                    "by_evidence_scope": {"original_cpp": 1},
                    "by_confidence": {"exact": 1, "tolerance": 0},
                    "rejected_records": 0,
                },
            }
        )

        self.assertEqual(10.0, ladder["behavior_verified"]["percent"])
        self.assertEqual(
            {"original_cpp": 1}, ladder["behavior_verified"]["evidence"]["scope_counts"]
        )
        self.assertEqual(
            {"exact": 1, "tolerance": 0},
            ladder["behavior_verified"]["evidence"]["confidence_counts"],
        )

    def test_dashboard_ladder_without_evidence_stays_compatible(self) -> None:
        ladder = port_dashboard.confidence_ladder(
            {
                "required_translation_units": 10,
                "reachable_implementation_candidates": 5,
                "reviewed_path_implementations": 2,
                "behavior_verified": 0,
                "blockers_by_kind": {},
            }
        )

        self.assertEqual(0.0, ladder["behavior_verified"]["percent"])
        self.assertNotIn("evidence", ladder["behavior_verified"])


if __name__ == "__main__":
    unittest.main()
