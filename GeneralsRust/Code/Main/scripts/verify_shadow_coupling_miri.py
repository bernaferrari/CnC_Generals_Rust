#!/usr/bin/env python3
"""Miri audit of the exact production scoped-pointer mechanism.

Extracts the coupling core verbatim into a dependency-free harness. The tiny
owner stands in for GameWorldShadow to isolate lifetime/aliasing behavior; this
does not validate gameplay, whole-world isolation, or C++ parity.
"""

from __future__ import annotations

import argparse
import hashlib
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
SOURCE = ROOT / "GeneralsRust/Code/Main/src/gameworld_shadow/tick/couple.rs"
DRIVER = r'''
fn access() -> Option<i32> { with_coupled_shadow_slot(|s| s.value) }
fn main() {
    assert_eq!(access(), None);
    let mut outer = Box::new(GameWorldShadow { value: 11 });
    let mut inner = GameWorldShadow { value: 22 };
    let result = with_coupled_shadow(&mut outer, || {
        assert_eq!(access(), Some(11));
        with_coupled_shadow_slot(|owner| {
            owner.value += 1;
            assert_eq!(access(), None);
            with_coupled_shadow(&mut inner, || {
                assert_eq!(access(), Some(22));
                with_coupled_shadow_slot(|s| s.value += 1).unwrap();
            });
            assert_eq!(access(), None);
        }).unwrap();
        assert_eq!(access(), Some(12));
        with_coupled_shadow_slot(|owner| {
            with_coupled_shadow(owner, || {
                assert_eq!(access(), Some(12));
                with_coupled_shadow_slot(|s| s.value = 12).unwrap();
            });
            assert_eq!(access(), None);
            assert_eq!(owner.value, 12);
        }).unwrap();
        assert_eq!(access(), Some(12));
        let nested_panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_coupled_shadow(&mut inner, || {
                with_coupled_shadow_slot(|_| panic!("nested borrow unwind"));
            });
        }));
        assert!(nested_panic.is_err());
        assert_eq!(access(), Some(12));
        42
    });
    assert_eq!(result, 42);
    assert_eq!(inner.value, 23);
    assert_eq!(access(), None);
    let moved_owner = *outer;
    assert_eq!(moved_owner.value, 12);
    drop(moved_owner);
    let mut next = GameWorldShadow { value: 33 };
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_coupled_shadow(&mut next, || panic!("owner scope unwind"));
    }));
    assert!(panic.is_err());
    assert_eq!(access(), None);
    with_coupled_shadow(&mut next, || assert_eq!(access(), Some(33)));
    assert_eq!(access(), None);
    println!("PASS: scoped coupling lifetime, nesting, reentry, movement and unwinding");
}
'''


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--toolchain", default="nightly")
    args = parser.parse_args()
    source = SOURCE.read_text()
    core = source[source.index("/// Temporary synchronous coupling slot."):source.index("/// Wave 680:")]
    assert "pub fn install_" not in core, "escaped pointer installation must not return"
    with tempfile.TemporaryDirectory(prefix="generals-scoped-shadow-") as directory:
        root = Path(directory)
        (root / "src").mkdir()
        (root / "Cargo.toml").write_text(
            '[package]\nname="scoped-shadow-audit"\nversion="0.1.0"\nedition="2024"\n[workspace]\n'
        )
        (root / "src/main.rs").write_text(
            "pub struct GameWorldShadow { value: i32 }\n" + core + DRIVER
        )
        subprocess.run(
            ["cargo", f"+{args.toolchain}", "miri", "run", "--manifest-path", str(root / "Cargo.toml")],
            check=True,
            timeout=180,
        )
    print(f"Production core sha256: {hashlib.sha256(core.encode()).hexdigest()}")


if __name__ == "__main__":
    main()
