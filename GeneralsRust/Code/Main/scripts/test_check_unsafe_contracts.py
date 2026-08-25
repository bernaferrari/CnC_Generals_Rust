from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))

from check_unsafe_contracts import UnsafeFile, inventory, strip_comments_and_literals, violations


class UnsafeContractRatchetTests(unittest.TestCase):
    def test_scanner_ignores_comments_and_literals_and_classifies_constructs(self) -> None:
        source = '''
// unsafe { ignored(); }
const TEXT: &str = "unsafe fn ignored()";
/* unsafe impl Ignored {} */
// SAFETY: pointer belongs to the fixture allocation.
unsafe { read(); }
unsafe fn call() {}
unsafe impl Send for Value {}
unsafe extern "C" { fn ffi(); }
'''
        stripped = strip_comments_and_literals(source)
        self.assertNotIn("ignored", stripped)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / "src/runtime.rs"
            path.parent.mkdir()
            path.write_text(source)
            files = inventory(root)
        self.assertEqual(1, len(files))
        self.assertEqual(4, files[0].constructs)
        self.assertEqual(3, files[0].undocumented)

    def test_ratchet_rejects_growth_new_files_and_stale_ceilings(self) -> None:
        baseline = {"src/a.rs": {"constructs": 2, "undocumented": 1}}
        grown = UnsafeFile("src/a.rs", "production", "runtime", 3, 1, (1, 2, 3))
        self.assertIn("constructs grew", violations([grown], baseline)[0])
        new = UnsafeFile("src/new.rs", "production", "runtime", 1, 0, (1,))
        self.assertIn("requires review", "\n".join(violations([new], {})))
        improved = UnsafeFile("src/a.rs", "production", "runtime", 2, 0, (1, 2))
        self.assertIn("ceiling must shrink", "\n".join(violations([improved], baseline)))


if __name__ == "__main__":
    unittest.main()
