#!/usr/bin/env python3
"""Check existence of critical global singletons in Rust codebase."""

import subprocess
from pathlib import Path

WORKSPACE = Path("/Users/bernardoferrari/Downloads/CnC_Generals_Zero_Hour-main")
RUST_DIR = WORKSPACE / "GeneralsRust"

# Critical global singletons from C++
SINGLETONS = ['TheAudio', 'TheGameLogic', 'TheGameClient', 'TheMessageStream']

print("Checking Rust global singleton definitions...\n")
for singleton in SINGLETONS:
    # Search for Rust-style versions (THE_* or *_in_rust)
    rust_name = singleton  # Often kept same or converted to SCREAMING_SNAKE_CASE
    result = subprocess.run(
        ['grep', '-r', f'THE_{singleton[3:].upper()}', RUST_DIR, '--include=*.rs'],
        capture_output=True, text=True
    )
    if result.returncode == 0:
        print(f"✓ Found {singleton} (as THE_{singleton[3:].upper()}):")
        for line in result.stdout.split('\n')[:3]:
            if line:
                print(f"    {line}")
    else:
        print(f"✗ Missing {singleton} (looking for THE_{singleton[3:].upper()})")
        
    # Also search for lowercase with getters
    result2 = subprocess.run(
        ['grep', '-r', f'get_{singleton.lower()}', RUST_DIR, '--include=*.rs'],
        capture_output=True, text=True
    )
    if result2.returncode == 0:
        print(f"  Also found getter function(s):")
        for line in result2.stdout.split('\n')[:2]:
            if line:
                print(f"    {line}")
    print()

print("\n\n=== Also checking for common accessor patterns ===")
for pattern in ['TheGameClient', 'TheAudio', 'TheMessageStream', 'TheGameLogic']:
    result = subprocess.run(
        ['grep', '-r', pattern, RUST_DIR, '--include=*.rs'],
        capture_output=True, text=True
    )
    if result.returncode == 0:
        lines = result.stdout.strip().split('\n')
        print(f"\n{pattern} appears {len(lines)} times in Rust:")
        for line in lines[:3]:
            print(f"  {line}")
