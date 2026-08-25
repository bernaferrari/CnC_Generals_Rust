from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))

from check_rust_loc import HARD_LIMIT, RustFile, is_test_path, violations


class RustLocRatchetTests(unittest.TestCase):
    def test_test_role_classification_is_path_based(self) -> None:
        self.assertTrue(is_test_path(Path("Code/Main/src/game_logic/world_tests/combat.rs")))
        self.assertTrue(is_test_path(Path("Code/Main/src/object/tests.rs")))
        self.assertTrue(is_test_path(Path("Code/Main/src/ai/cpp_parity_tests.rs")))
        self.assertTrue(is_test_path(Path("tests/integration_pathfinding.rs")))
        self.assertFalse(is_test_path(Path("Code/Main/src/executable_smoke.rs")))
        self.assertFalse(is_test_path(Path("Code/GameEngine/Common/src/testing_adapter.rs")))


    def test_new_oversized_file_fails(self) -> None:
        files = [RustFile("src/new.rs", HARD_LIMIT + 1, "production")]
        self.assertIn(
            "new oversized production file",
            violations(files, {"production": {}, "test": {}})[0],
        )


    def test_allowlisted_file_may_shrink_but_not_grow(self) -> None:
        allowlist = {"production": {"src/large.rs": 5_000}, "test": {}}
        self.assertEqual(
            [], violations([RustFile("src/large.rs", 4_500, "production")], allowlist)
        )
        self.assertIn(
            "LOC ratchet grew",
            violations([RustFile("src/large.rs", 5_001, "production")], allowlist)[0],
        )


    def test_allowlist_entry_must_be_removed_after_split(self) -> None:
        allowlist = {"production": {"src/large.rs": 5_000}, "test": {}}
        self.assertIn(
            "must leave allowlist",
            violations([RustFile("src/large.rs", HARD_LIMIT, "production")], allowlist)[0],
        )


if __name__ == "__main__":
    unittest.main()
