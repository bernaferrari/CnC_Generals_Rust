#!/usr/bin/env python3
"""Search for TheGameClient as a struct with static methods."""

import subprocess
from pathlib import Path

WORKSPACE = Path("/Users/bernardoferrari/Downloads/CnC_Generals_Zero_Hour-main")
RUST_DIR = WORKSPACE / "GeneralsRust" / "Code" / "GameEngine"

# Find definition of TheGameClient struct (not GameClient but TheGameClient specifically)
result = subprocess.run(
    ['grep', '-rn', 'struct TheGameClient', RUST_DIR, '--include=*.rs'],
    capture_output=True, text=True
)
print("struct TheGameClient:")
if result.returncode == 0:
    print(result.stdout[:500])
else:
    print("  Not found as a struct")

# Find enum with TheGameClient variant
result2 = subprocess.run(
    ['grep', '-rn', 'TheGameClient\s*=', RUST_DIR, '--include=*.rs'],
    capture_output=True, text=True
)
print("\nTheGameClient = :")
if result2.returncode == 0:
    print(result2.stdout[:500])
else:
    print("  Not found as an assignment")

# Check if TheGameClient is a module-level alias
result3 = subprocess.run(
    ['grep', '-rn', 'type TheGameClient', RUST_DIR, '--include=*.rs'],
    capture_output=True, text=True
)
print("\ntype TheGameClient:")
if result3.returncode == 0:
    print(result3.stdout[:500])
else:
    print("  Not found")

# Look for pub use of TheGameClient from prelude
prelude = RUST_DIR / "Precompiled" / "PreRTS.rs"
if prelude.exists():
    content = prelude.read_text()
    print("\n=== PreRTS.rs - checking for TheGameClient re-export ===")
    for i, line in enumerate(content.split('\n')):
        if 'TheGameClient' in line or 'GAME_CLIENT' in line:
            print(f"  Line {i}: {line}")

# Check lib.rs in Common
common_lib = RUST_DIR / "Common" / "src" / "lib.rs"
if common_lib.exists():
    content = common_lib.read_text()
    print("\n=== Common/lib.rs - checking for TheGameClient ===")
    for i, line in enumerate(content.split('\n')):
        if 'TheGameClient' in line or 'game_client' in line.lower():
            print(f"  Line {i}: {line}")
            
game_client_lib = RUST_DIR / "GameClient" / "src" / "lib.rs"
if game_client_lib.exists():
    content = game_client_lib.read_text()
    print("\n=== GameClient/lib.rs - checking for TheGameClient ===")
    for i, line in enumerate(content.split('\n')):
        if 'TheGameClient' in line or 'GAME_CLIENT' in line:
            print(f"  Line {i}: {line}")
