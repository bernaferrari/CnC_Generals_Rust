#!/usr/bin/env python3
"""Find TheGameLogic definition and get method."""

import subprocess
from pathlib import Path

WORKSPACE = Path("/Users/bernardoferrari/Downloads/CnC_Generals_Zero_Hour-main")
RUST_DIR = WORKSPACE / "GeneralsRust" / "Code" / "GameEngine"

# Search for TheGameLogic struct
result = subprocess.run(
    ['grep', '-rn', 'struct TheGameLogic', RUST_DIR, '--include=*.rs'],
    capture_output=True, text=True
)
print("struct TheGameLogic:")
if result.returncode == 0:
    print(result.stdout[:1000])
else:
    print("  Not found")

# Search for TheGameLogic get method
result2 = subprocess.run(
    ['grep', '-rn', 'impl TheGameLogic', RUST_DIR, '--include=*.rs'],
    capture_output=True, text=True
)
print("\nimpl TheGameLogic:")
if result2.returncode == 0:
    print(result2.stdout[:1000])
else:
    print("  Not found")

# Search for TheGameLogic static
result3 = subprocess.run(
    ['grep', '-rn', 'static.*THE_GAME_LOGIC', RUST_DIR, '--include=*.rs'],
    capture_output=True, text=True
)
print("\nstatic THE_GAME_LOGIC:")
if result3.returncode == 0:
    print(result3.stdout[:500])
else:
    print("  Not found")

# Check for module that might contain TheGameLogic
result4 = subprocess.run(
    ['grep', '-rn', 'TheGameLogic::get', RUST_DIR, '--include=*.rs'],
    capture_output=True, text=True
)
print("\nTheGameLogic::get() calls:")
if result4.returncode == 0:
    for line in result4.stdout.strip().split('\n')[:10]:
        print(f"  {line}")
else:
    print("  Not found")

# Check common/game_engine.rs for TheGameLogic pattern
game_engine = RUST_DIR / "Common" / "src" / "common" / "game_engine.rs"
if game_engine.exists():
    content = game_engine.read_text()
    print("\n=== TheGameLogic mentions in game_engine.rs ===")
    for i, line in enumerate(content.split('\n')):
        if 'TheGameLogic' in line:
            print(f"  Line {i+1}: {line}")
