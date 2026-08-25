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


if __name__ == "__main__":
    unittest.main()
