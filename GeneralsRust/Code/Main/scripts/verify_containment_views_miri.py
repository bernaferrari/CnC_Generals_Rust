#!/usr/bin/env python3
"""Audit the exact production containment snapshot method under Miri.

The adapter replaces gameplay with an ordered ID vector to isolate query
lifetimes. It does not verify the whole containment engine or C++ saves.
An optional old revision reproduces the former cache's safe-call UAF.
"""

from __future__ import annotations

import argparse
import hashlib
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
PRISON = "GeneralsRust/Code/GameEngine/GameLogic/src/object/behavior/prison_behavior.rs"
CAVE = "GeneralsRust/Code/GameEngine/GameLogic/src/object/contain/cave_contain.rs"


def method_body(source: str) -> str:
    section = source[source.index("impl ContainModuleInterface for PrisonBehaviorContainHandle") :]
    start = section.index("    fn get_contained_objects(")
    brace = section.index("{", start)
    depth = 1
    end = brace + 1
    while depth:
        depth += (section[end] == "{") - (section[end] == "}")
        end += 1
    return section[start:end]


def run_miri(source: str, toolchain: str) -> subprocess.CompletedProcess[str]:
    with tempfile.TemporaryDirectory(prefix="generals-containment-audit-") as directory:
        root = Path(directory)
        (root / "src").mkdir()
        (root / "Cargo.toml").write_text(
            '[package]\nname="containment-view-audit"\nversion="0.1.0"\nedition="2024"\n[workspace]\n'
        )
        (root / "src/main.rs").write_text(source)
        return subprocess.run(
            ["cargo", f"+{toolchain}", "miri", "run", "--manifest-path", str(root / "Cargo.toml")],
            capture_output=True,
            text=True,
            timeout=180,
        )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--toolchain", default="nightly")
    parser.add_argument("--negative-revision", help="Old Git revision containing SharedContainIdCache")
    args = parser.parse_args()
    if args.negative_revision:
        old = subprocess.check_output(
            ["git", "show", f"{args.negative_revision}:{CAVE}"], cwd=ROOT, text=True
        )
        start = old.index("#[derive(Debug)]\nstruct SharedContainIdCache")
        core = old[start : old.index("/// Configuration data for CaveContain", start)]
        result = run_miri(
            "use std::cell::UnsafeCell;\ntype ObjectID = u32;\n" + core + """
fn main() {
    let cache = SharedContainIdCache::new();
    let first = cache.refresh(vec![11, 22]);
    let second = cache.refresh(vec![33, 44]);
    assert_eq!(second, &[33, 44]);
    assert_eq!(first, &[11, 22]);
}
""",
            args.toolchain,
        )
        assert result.returncode != 0 and "use-after-free" in result.stderr, result.stderr
        print("CONFIRMED: old safe cache refresh leaves a dangling slice")

    core = method_body((ROOT / PRISON).read_text())
    assert "Cow<'_, [ObjectID]>" in core
    adapter = """
use std::borrow::Cow;
use std::sync::{Arc, Mutex};
type ObjectID = u32;
struct Behavior { ids: Vec<ObjectID> }
impl Behavior {
    fn get_contained_objects(&self) -> Cow<'_, [ObjectID]> { Cow::Borrowed(&self.ids) }
}
struct PrisonBehaviorContainHandle { behavior: Arc<Mutex<Behavior>> }
impl PrisonBehaviorContainHandle {
""" + core + """
}
fn main() {
    let behavior = Arc::new(Mutex::new(Behavior { ids: vec![11, 22] }));
    let handle = Mutex::new(PrisonBehaviorContainHandle { behavior: behavior.clone() });
    let guard = handle.lock().unwrap();
    let first = guard.get_contained_objects();
    behavior.lock().unwrap().ids = vec![33, 44];
    let second = guard.get_contained_objects();
    assert_eq!(first.as_ref(), &[11, 22]);
    assert_eq!(second.as_ref(), &[33, 44]);
    assert!(matches!(first, Cow::Owned(_)));
    let other = PrisonBehaviorContainHandle {
        behavior: Arc::new(Mutex::new(Behavior { ids: vec![55] }))
    };
    assert_eq!(other.get_contained_objects().as_ref(), &[55]);
    assert_eq!(guard.get_contained_objects().as_ref(), &[33, 44]);
    behavior.lock().unwrap().ids.clear();
    assert!(guard.get_contained_objects().is_empty());
    assert_eq!(first.as_ref(), &[11, 22]);
    println!("PASS: retained views, fresh ordered queries and independent storage");
}
"""
    result = run_miri(adapter, args.toolchain)
    print(result.stdout, end="")
    if result.returncode:
        raise RuntimeError(result.stderr)
    print(f"Production method sha256: {hashlib.sha256(core.encode()).hexdigest()}")


if __name__ == "__main__":
    main()
