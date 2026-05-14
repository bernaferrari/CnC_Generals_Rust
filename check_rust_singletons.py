#!/usr/bin/env python3
"""Find global singleton definitions in Rust code."""

import subprocess
from pathlib import Path

WORKSPACE = Path("/Users/bernardoferrari/Downloads/CnC_Generals_Zero_Hour-main")
RUST_DIR = WORKSPACE / "GeneralsRust"

# Search for patterns indicating global singleton definitions
patterns = [
    'THE_GAMECLIENT',
    'THE_GAME_LOGIC', 
    'THE_GAME_LOGIC',
    'GAME_LOGIC',
    'get_game_client',
    'get_game_logic',
    'get_message_stream',
    'TheGameClient::get',
    'TheGameLogic::get',
]

for pattern in patterns:
    result = subprocess.run(
        ['grep', '-r', pattern, RUST_DIR, '--include=*.rs', '-n'],
        capture_output=True, text=True
    )
    if result.returncode == 0:
        print(f"\n=== Pattern: {pattern} ===")
        lines = result.stdout.strip().split('\n')
        for line in lines[:5]:  # Show first 5 matches
            print(f"  {line}")

print("\n\n=== Checking for lazy_static/lazy/OnceLock declarations of main globals ===")
for pattern in ['lazy_static!', 'lazy!', 'OnceLock::new']:
    result = subprocess.run(
        ['grep', '-r', pattern, RUST_DIR, '--include=*.rs', '-A', '2'],
        capture_output=True, text=True
    )
    if result.returncode == 0:
        lines = result.stdout.strip().split('\n')
        print(f"\n{pattern} usage:")


print("\n\n=== Searching for TheGameClient struct and impl ===")        
result = subprocess.run(['grep', '-r', 'struct GameClient', RUST_DIR, '--include=*.rs'], capture_output=True, text=True)
if result.returncode == 0:
    for line in result.stdout.strip().split('\n')[:3]:
        print(f"  {line}")

result2 = subprocess.run(['grep', '-r', 'impl.*GameClient', RUST_DIR, '--include=*.rs'], capture_output=True, text=True)
if result2.returncode == 0:
    lines = result2.stdout.strip().split('\n')[:3]
    print("  impl blocks:")
    for line in lines:
        print(f"    {line}")
        
print("\n\n=== Checking for 'pub struct GameLogic' ===")
result3 = subprocess.run(['grep', '-r', 'pub struct GameLogic', RUST_DIR, '--include=*.rs'], capture_output=True, text=True)
if result3.returncode == 0:
    print(result3.stdout)
else:
    print("  No 'pub struct GameLogic' found. Might be in a different module.")
    # Search for GameLogic struct at all
    result4 = subprocess.run(['grep', '-r', 'struct GameLogic', RUST_DIR, '--include=*.rs'], capture_output=True, text=True)
    if result4.returncode == 0:
        print("  Found 'struct GameLogic':")
        print(result4.stdout[:500])
    else:
        print("  No 'struct GameLogic' anywhere.")
