from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))

import validate_rust_split


class ValidateRustSplitTests(unittest.TestCase):
    def make_repo(self) -> Path:
        root = Path(tempfile.mkdtemp())
        subprocess.run(["git", "init", "-q"], cwd=root, check=True)
        subprocess.run(["git", "config", "user.email", "split@example.invalid"], cwd=root, check=True)
        subprocess.run(["git", "config", "user.name", "Split Test"], cwd=root, check=True)
        crate = root / "GeneralsRust/Code/Main"
        (crate / "src").mkdir(parents=True)
        (root / "GeneralsRust/Cargo.toml").write_text("[workspace]\nmembers=[]\n")
        (crate / "Cargo.toml").write_text('[package]\nname="fixture"\nversion="0.1.0"\n')
        return root

    def commit_source(self, root: Path, text: str) -> Path:
        source = Path("GeneralsRust/Code/Main/src/large.rs")
        (root / source).write_text(text)
        subprocess.run(["git", "add", "."], cwd=root, check=True)
        subprocess.run(["git", "commit", "-qm", "baseline"], cwd=root, check=True)
        return source

    def make_extracted_dependency(self, root: Path, tests: str = "") -> Path:
        dependency = root / "GeneralsRust/Code/GameEngine/Extracted"
        (dependency / "src").mkdir(parents=True)
        (dependency / "Cargo.toml").write_text(
            '[package]\nname="extracted"\nversion="0.1.0"\n'
        )
        (dependency / "src/lib.rs").write_text(
            "mod events;\npub use events::Stable;\n#[cfg(test)]\nmod tests;\n"
        )
        (dependency / "src/events.rs").write_text("pub struct Stable;\n")
        (dependency / "src/tests.rs").write_text(tests)
        main_manifest = root / "GeneralsRust/Code/Main/Cargo.toml"
        main_manifest.write_text(
            '[package]\nname="fixture"\nversion="0.1.0"\n'
            '[dependencies]\nextracted={path="../GameEngine/Extracted"}\n'
        )
        return dependency

    def test_cohesive_split_preserves_tests_and_public_surface(self) -> None:
        root = self.make_repo()
        source = self.commit_source(root, "pub struct Stable;\n#[test]\nfn behavior() {}\n")
        (root / source).unlink()
        module = (root / source).with_suffix("")
        module.mkdir()
        (module / "mod.rs").write_text("mod behavior;\npub struct Stable;\n")
        (module / "behavior.rs").write_text("#[test]\nfn behavior() {}\n")
        report = validate_rust_split.validate(root, root / "GeneralsRust", source, "HEAD")
        self.assertTrue(report["passed"], report["problems"])
        self.assertEqual("fixture", report["package"])

    def test_rejects_numbered_shard_lost_test_and_public_growth(self) -> None:
        root = self.make_repo()
        source = self.commit_source(root, "pub struct Stable;\n#[test]\nfn behavior() {}\n")
        (root / source).unlink()
        module = (root / source).with_suffix("")
        module.mkdir()
        (module / "mod.rs").write_text("mod part_1;\npub struct Stable;\npub fn Leak() {}\n")
        (module / "part_1.rs").write_text("fn behavior() {}\n")
        report = validate_rust_split.validate(root, root / "GeneralsRust", source, "HEAD")
        joined = "\n".join(report["problems"])
        self.assertIn("mechanical numeric shard", joined)
        self.assertIn("test attributes decreased", joined)
        self.assertIn("new public API names: Leak", joined)

    def test_nested_literal_source_includes_preserve_tests_and_surface(self) -> None:
        root = self.make_repo()
        source = self.commit_source(root, "pub struct Stable;\n#[test]\nfn behavior() {}\n")
        (root / source).write_text('include!("parts.rs");\n')
        (root / source).parent.joinpath("parts.rs").write_text('include!("nested/behavior.rs");\n')
        nested = (root / source).parent / "nested"
        nested.mkdir()
        (nested / "behavior.rs").write_text("pub struct Stable;\n#[test]\nfn behavior() {}\n")
        report = validate_rust_split.validate(root, root / "GeneralsRust", source, "HEAD")
        self.assertTrue(report["passed"], report["problems"])
        self.assertEqual({"before": 1, "after": 1}, report["tests"])
        self.assertEqual(3, len(report["fragments"]))

    def test_literal_source_include_cannot_hide_oversized_fragment(self) -> None:
        root = self.make_repo()
        source = self.commit_source(root, "pub struct Stable;\n")
        (root / source).write_text('pub struct Stable;\ninclude!("behavior.rs");\n')
        (root / source).parent.joinpath("behavior.rs").write_text(
            "// body\n" * (validate_rust_split.HARD_LIMIT + 1)
        )
        report = validate_rust_split.validate(root, root / "GeneralsRust", source, "HEAD")
        self.assertFalse(report["passed"])
        self.assertTrue(any("oversized fragment" in problem for problem in report["problems"]))

    def test_literal_source_include_cannot_hide_public_growth(self) -> None:
        root = self.make_repo()
        source = self.commit_source(root, "pub struct Stable;\n")
        (root / source).write_text('pub struct Stable;\ninclude!("behavior.rs");\n')
        (root / source).parent.joinpath("behavior.rs").write_text("pub fn Leak() {}\n")
        report = validate_rust_split.validate(root, root / "GeneralsRust", source, "HEAD")
        self.assertFalse(report["passed"])
        self.assertIn("new public API names: Leak", report["problems"])

    def test_literal_data_include_is_not_a_rust_source_edge(self) -> None:
        root = self.make_repo()
        source = self.commit_source(root, "pub struct Stable;\n")
        (root / source).write_text('pub struct Stable;\nconst DATA: &str = include_str!("data.rs");\n')
        (root / source).parent.joinpath("data.rs").write_text("pub fn Leak() {}\n#[test]\nfn unrelated() {}\n")
        report = validate_rust_split.validate(root, root / "GeneralsRust", source, "HEAD")
        self.assertTrue(report["passed"], report["problems"])
        self.assertEqual({"before": 0, "after": 0}, report["tests"])
        self.assertEqual(1, len(report["fragments"]))

    def test_rejects_literal_include_of_removed_monolith(self) -> None:
        root = self.make_repo()
        source = self.commit_source(root, "pub struct Stable;\n")
        (root / source).unlink()
        module = (root / source).with_suffix("")
        module.mkdir()
        (module / "mod.rs").write_text("mod behavior;\npub struct Stable;\n")
        (module / "behavior.rs").write_text("fn behavior() {}\n")
        consumer = root / "GeneralsRust/Code/Main/src/consumer.rs"
        consumer.write_text('const OLD: &str = include_str!("large.rs");\n')

        report = validate_rust_split.validate(root, root / "GeneralsRust", source, "HEAD")

        self.assertFalse(report["passed"])
        self.assertEqual(
            ["GeneralsRust/Code/Main/src/consumer.rs"],
            report["stale_source_references"],
        )

    def test_referenced_extracted_crate_root_includes_its_declared_tests(self) -> None:
        root = self.make_repo()
        source = self.commit_source(root, "pub struct Stable;\n#[test]\nfn behavior() {}\n")
        extracted = self.make_extracted_dependency(
            root, "#[test]\nfn behavior() {}\n"
        )
        (root / source).write_text("pub use extracted::Stable;\n")

        report = validate_rust_split.validate(
            root,
            root / "GeneralsRust",
            source,
            "HEAD",
            [Path("GeneralsRust/Code/GameEngine/Extracted/src/events.rs"),
             Path("GeneralsRust/Code/GameEngine/Extracted/src/tests.rs")],
        )

        self.assertTrue(report["passed"], report["problems"])
        self.assertEqual({"before": 1, "after": 1}, report["tests"])
        self.assertIn(
            "GeneralsRust/Code/GameEngine/Extracted/src/tests.rs",
            {fragment["path"] for fragment in report["fragments"]},
        )

    def test_reachable_extracted_crate_still_rejects_lost_tests(self) -> None:
        root = self.make_repo()
        source = self.commit_source(root, "pub struct Stable;\n#[test]\nfn behavior() {}\n")
        extracted = self.make_extracted_dependency(root)
        (extracted / "src/events.rs").write_text("pub struct Stable;\npub fn Leak() {}\n")
        (root / source).write_text("pub use extracted::Stable;\n")

        report = validate_rust_split.validate(
            root,
            root / "GeneralsRust",
            source,
            "HEAD",
            [Path("GeneralsRust/Code/GameEngine/Extracted/src/events.rs")],
        )

        self.assertFalse(report["passed"])
        self.assertIn("test attributes decreased: 1 -> 0", report["problems"])
        self.assertIn("new public API names: Leak", report["problems"])

    def test_reachable_sibling_tests_cannot_mask_lost_mapped_tests(self) -> None:
        root = self.make_repo()
        source = self.commit_source(root, "pub struct Stable;\n#[test]\nfn behavior() {}\n")
        self.make_extracted_dependency(root, "#[test]\nfn unrelated_behavior() {}\n")
        (root / source).write_text("pub use extracted::Stable;\n")
        report = validate_rust_split.validate(
            root, root / "GeneralsRust", source, "HEAD",
            [Path("GeneralsRust/Code/GameEngine/Extracted/src/events.rs")],
        )
        self.assertFalse(report["passed"])
        self.assertIn("test attributes decreased: 1 -> 0", report["problems"])

    def test_unreachable_unrelated_file_cannot_supply_tests(self) -> None:
        root = self.make_repo()
        source = self.commit_source(root, "pub struct Stable;\n#[test]\nfn behavior() {}\n")
        self.make_extracted_dependency(root)
        (root / source).write_text("pub use extracted::Stable;\n")
        unrelated = root / "GeneralsRust/Code/GameEngine/Extracted/src/unrelated.rs"
        unrelated.write_text("#[test]\nfn behavior() {}\n")

        report = validate_rust_split.validate(
            root,
            root / "GeneralsRust",
            source,
            "HEAD",
            [Path("GeneralsRust/Code/GameEngine/Extracted/src/unrelated.rs")],
        )

        self.assertFalse(report["passed"])
        self.assertTrue(
            any("not reachable from a declared local path dependency" in p for p in report["problems"])
        )
        self.assertIn("test attributes decreased: 1 -> 0", report["problems"])

    def test_expected_crate_api_accepts_exact_internal_declaration(self) -> None:
        self.assertEqual(
            (Path("GeneralsRust/Code/Main/src/large/driver.rs"), "Driver"),
            validate_rust_split.parse_expected_crate_api(
                "GeneralsRust/Code/Main/src/large/driver.rs:Driver"
            ),
        )
        root = self.make_repo()
        source = self.commit_source(root, "pub struct Stable;\n#[test]\nfn behavior() {}\n")
        (root / source).unlink()
        module = (root / source).with_suffix("")
        module.mkdir()
        (module / "mod.rs").write_text("mod driver;\npub struct Stable;\n")
        (module / "driver.rs").write_text(
            "pub(crate) struct Driver;\n#[test]\nfn behavior() {}\n"
        )

        strict = validate_rust_split.validate(root, root / "GeneralsRust", source, "HEAD")
        self.assertFalse(strict["passed"])
        self.assertIn("new public API names: Driver", strict["problems"])

        report = validate_rust_split.validate(
            root,
            root / "GeneralsRust",
            source,
            "HEAD",
            expected_crate_api=[(Path("GeneralsRust/Code/Main/src/large/driver.rs"), "Driver")],
        )

        self.assertTrue(report["passed"], report["problems"])
        self.assertEqual(["Driver"], report["public_api"]["added"])
        self.assertEqual(
            [{"path": "GeneralsRust/Code/Main/src/large/driver.rs", "name": "Driver", "accepted": True}],
            report["public_api"]["expected_crate_api"]["accepted"],
        )

    def test_expected_crate_api_rejects_external_declaration_and_reexport(self) -> None:
        for declaration, root_module in (
            ("pub struct Driver;", "mod driver;\npub struct Stable;\n"),
            ("pub(crate) struct Driver;", "mod driver;\npub struct Stable;\npub use driver::Driver;\n"),
        ):
            with self.subTest(declaration=declaration, root_module=root_module):
                root = self.make_repo()
                source = self.commit_source(
                    root, "pub struct Stable;\n#[test]\nfn behavior() {}\n"
                )
                (root / source).unlink()
                module = (root / source).with_suffix("")
                module.mkdir()
                (module / "mod.rs").write_text(root_module)
                (module / "driver.rs").write_text(
                    f"{declaration}\n#[test]\nfn behavior() {{}}\n"
                )

                report = validate_rust_split.validate(
                    root,
                    root / "GeneralsRust",
                    source,
                    "HEAD",
                    expected_crate_api=[
                        (Path("GeneralsRust/Code/Main/src/large/driver.rs"), "Driver")
                    ],
                )

                self.assertFalse(report["passed"])
                self.assertIn("new public API names: Driver", report["problems"])
                self.assertTrue(report["public_api"]["added"])
                self.assertFalse(report["public_api"]["expected_crate_api"]["accepted"])
                self.assertTrue(
                    any("one unique pub(crate) declaration" in p for p in report["problems"])
                )

    def test_expected_crate_api_rejects_stale_wrongfile_and_unknown_path(self) -> None:
        root = self.make_repo()
        source = self.commit_source(
            root, "pub struct Stable;\n#[test]\nfn behavior() {}\n"
        )
        (root / source).unlink()
        module = (root / source).with_suffix("")
        module.mkdir()
        (module / "mod.rs").write_text("mod driver;\nmod other;\npub struct Stable;\n")
        (module / "driver.rs").write_text(
            "pub(crate) struct Driver;\n#[test]\nfn behavior() {}\n"
        )
        (module / "other.rs").write_text("\n")
        unrelated = root / "GeneralsRust/Code/Main/src/unreachable.rs"
        unrelated.write_text("pub(crate) struct Driver;\n")
        driver = Path("GeneralsRust/Code/Main/src/large/driver.rs")
        other = Path("GeneralsRust/Code/Main/src/large/other.rs")
        unknown = Path("GeneralsRust/Code/Main/src/unreachable.rs")

        stale_root = self.make_repo()
        stale_source = self.commit_source(
            stale_root,
            "pub struct Stable;\npub(crate) struct Driver;\n#[test]\nfn behavior() {}\n",
        )
        (stale_root / stale_source).unlink()
        stale_module = (stale_root / stale_source).with_suffix("")
        stale_module.mkdir()
        (stale_module / "mod.rs").write_text("mod driver;\npub struct Stable;\n")
        (stale_module / "driver.rs").write_text(
            "pub(crate) struct Driver;\n#[test]\nfn behavior() {}\n"
        )
        stale = validate_rust_split.validate(
            stale_root,
            stale_root / "GeneralsRust",
            stale_source,
            "HEAD",
            expected_crate_api=[(driver, "Driver")],
        )
        self.assertFalse(stale["passed"])
        self.assertIn("stale, already present before split", "\n".join(stale["problems"]))

        wrong_file = validate_rust_split.validate(
            root, root / "GeneralsRust", source, "HEAD", expected_crate_api=[(other, "Driver")]
        )
        self.assertFalse(wrong_file["passed"])
        self.assertIn("named fragment", "\n".join(wrong_file["problems"]))

        unknown_path = validate_rust_split.validate(
            root, root / "GeneralsRust", source, "HEAD", expected_crate_api=[(unknown, "Driver")]
        )
        self.assertFalse(unknown_path["passed"])
        self.assertIn("not an in-scope current fragment", "\n".join(unknown_path["problems"]))

        missing = validate_rust_split.validate(
            root, root / "GeneralsRust", source, "HEAD", expected_crate_api=[(driver, "Missing")]
        )
        self.assertFalse(missing["passed"])
        self.assertIn("declaration is missing", "\n".join(missing["problems"]))

        duplicate_request = validate_rust_split.validate(
            root, root / "GeneralsRust", source, "HEAD",
            expected_crate_api=[(driver, "Driver"), (driver, "Driver")],
        )
        self.assertFalse(duplicate_request["passed"])
        self.assertIn("duplicate expected crate API request", "\n".join(duplicate_request["problems"]))

    def test_expected_crate_api_rejects_duplicate_shadow_names(self) -> None:
        root = self.make_repo()
        source = self.commit_source(root, "pub struct Stable;\n#[test]\nfn behavior() {}\n")
        (root / source).unlink()
        module = (root / source).with_suffix("")
        module.mkdir()
        (module / "mod.rs").write_text("mod driver;\nmod shadow;\npub struct Stable;\n")
        (module / "driver.rs").write_text(
            "pub(crate) struct Driver;\n#[test]\nfn behavior() {}\n"
        )
        (module / "shadow.rs").write_text("pub(crate) struct Driver;\n")

        report = validate_rust_split.validate(
            root,
            root / "GeneralsRust",
            source,
            "HEAD",
            expected_crate_api=[(Path("GeneralsRust/Code/Main/src/large/driver.rs"), "Driver")],
        )

        self.assertFalse(report["passed"])
        self.assertIn("one unique pub(crate) declaration", "\n".join(report["problems"]))
        self.assertFalse(report["public_api"]["expected_crate_api"]["accepted"])


if __name__ == "__main__":
    unittest.main()
