#!/usr/bin/env python3
"""Find TheGameClient::get() and TheGameLogic::get() definitions."""

import subprocess
from pathlib import Path

WORKSPACE = Path("/Users/bernardoferrari/Downloads/CnC_Generals_Zero_Hour-main")
RUST_DIR = WORKSPACE / "GeneralsRust" / "Code" / "GameEngine"

# Search for function definitions matching TheGameClient::get or TheGameLogic::get
patterns = [
    r'pub fn get\(',
    r'fn get\(',
    r'TheGameClient\s*{',
    r'TheGameLogic\s*{',
]

# Also check common headers to see if there's a mod game_client; that re-exports
result = subprocess.run(
    ['find', RUST_DIR, '-name', 'mod.rs', '-o', '-name', 'lib.rs'],
    capture_output=True, text=True
)
print("Main module files:")
for line in result.stdout.strip().split('\n')[:10]:
    if line:
        print(f"  {line}")

# Check the main prelude
prelude = RUST_DIR / "Common" / "src" / "common" / "prelude.rs"
if prelude.exists():
    print("\n=== prelude.rs ===")
    print(prelude.read_text()[:2000])

# Check the game_engine.rs which might have the globals
game_engine = RUST_DIR / "Common" / "src" / "common" / "game_engine.rs"
if game_engine.exists():
    print("\n=== game_engine.rs (first 200 lines) ===")
    print(game_engine.read_text()[:4000])
