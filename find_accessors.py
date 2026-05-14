#!/usr/bin/env python3
"""Locate TheGameClient and TheGameLogic accessors in Rust."""

import subprocess
from pathlib import Path

WORKSPACE = Path("/Users/bernardoferrari/Downloads/CnC_Generals_Zero_Hour-main")
RUST_DIR = WORKSPACE / "GeneralsRust" / "Code" / "GameEngine"

# Find the function definition for TheGameClient::get
result = subprocess.run(
    ['grep', '-rn', 'TheGameClient::get', RUST_DIR, '--include=*.rs'],
    capture_output=True, text=True
)
print("TheGameClient::get() references:")
if result.returncode == 0:
    for line in result.stdout.strip().split('\n')[:10]:
        print(f"  {line}")
else:
    print("  None found")

# Look for GameClient as a singleton type with get method
result2 = subprocess.run(
    ['grep', '-rn', 'impl.*GameClient', RUST_DIR, '--include=*.rs'],
    capture_output=True, text=True
)
print("\nimpl blocks for GameClient:")
if result2.returncode == 0:
    for line in result2.stdout.strip().split('\n')[:10]:
        print(f"  {line}")
        
# Check helpers module
helpers_path = RUST_DIR / "Common" / "src" / "common" / "helpers.rs"
if helpers_path.exists():
    content = helpers_path.read_text()
    print("\n\n=== helpers.rs content (first 200 lines) ===")
    print(content[:2000])
    
# Check main integration file  
main_rs = RUST_DIR / "Precompiled" / "PreRTS.rs"
if main_rs.exists():
    content = main_rs.read_text()
    print("\n\n=== PreRTS.rs (first 150 lines) ===")
    print(content[:3000])
