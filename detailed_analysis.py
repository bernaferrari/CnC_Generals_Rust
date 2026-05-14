#!/usr/bin/env python3
"""
Detailed analysis of critical global singleton usage patterns.
"""

import subprocess
import re
from pathlib import Path
from collections import defaultdict

WORKSPACE = Path("/Users/bernardoferrari/Downloads/CnC_Generals_Zero_Hour-main")
RUST_DIR = WORKSPACE / "GeneralsRust" / "Code" / "GameEngine"

# Files that were flagged as critical in the mismatch report
CRITICAL_FILES = [
    "GameEngine.cpp",
    "GameAudio.cpp",
    "ControlBar.cpp",
    "Object.cpp",
    "GameLogic.cpp",
]

# Files that were flagged as high severity (missing init)
HIGH_INIT_FILES = [
    "GameAudio.cpp",
    "GameEngine.cpp",
    "GameMain.cpp",
    "GlobalData.cpp",
    "MessageStream.cpp",
    "SubsystemInterface.cpp",
    "ThingFactory.cpp",
    "Keyboard.cpp",
    "Mouse.cpp",
    "HotKey.cpp",
    "WOLLoginMenu.cpp",
]

# Global singletons we expect to find
SINGLETONS = {
    'TheAudio': 'THE_AUDIO',
    'TheGameLogic': 'TheGameLogic',
    'TheGameClient': 'TheGameClient',
    'TheMessageStream': 'THE_MESSAGE_STREAM',
}

print("=" * 80)
print("CRITICAL GLOBAL SINGLETON ANALYSIS")
print("=" * 80)

for cpp_name in CRITICAL_FILES:
    print(f"\n--- Checking {cpp_name} ---")
    
    # Find the corresponding Rust file
    result = subprocess.run(
        ['grep', '-r', cpp_name, WORKSPACE / 'work_list.txt'],
        capture_output=True, text=True
    )
    if result.returncode != 0:
        print(f"  Not found in work_list.txt")
        continue
    
    line = result.stdout.strip()
    parts = line.split('|')
    if len(parts) < 2:
        continue
    
    rust_rel = parts[1].strip()
    rust_path = WORKSPACE / rust_rel
    
    if not rust_path.exists():
        print(f"  Rust file not found: {rust_rel}")
        continue
    
    rust_content = rust_path.read_text(encoding='utf-8', errors='ignore')
    rust_filename = rust_path.name
    
    print(f"  Rust file: {rust_filename}")
    
    # Check each singleton
    for cpp_singleton, rust_pattern in SINGLETONS.items():
        # Does C++ file reference this singleton?
        cpp_path = WORKSPACE / parts[0].strip()
        if cpp_path.exists():
            cpp_content = cpp_path.read_text(encoding='utf-8', errors='ignore')
            cpp_uses = cpp_singleton in cpp_content
        else:
            cpp_uses = False
        
        # Does Rust file reference this singleton?
        rust_uses = False
        if rust_pattern in rust_content:
            rust_uses = True
        else:
            # Check for TheGameClient::get() pattern
            if rust_pattern == 'TheGameClient' and 'TheGameClient::get()' in rust_content:
                rust_uses = True
            elif rust_pattern == 'TheGameLogic' and 'TheGameLogic::' in rust_content:
                rust_uses = True
        
        if cpp_uses and not rust_uses:
            print(f"  ⚠️  WARNING: C++ uses '{cpp_singleton}' but Rust file doesn't appear to use '{rust_pattern}'")
        elif cpp_uses and rust_uses:
            print(f"  ✓ {cpp_singleton} found in both")

print("\n\n" + "=" * 80)
print("INIT FUNCTION ANALYSIS")
print("=" * 80)

# For each high-priority file, check if it has an init() implementation matching the C++ signature
for cpp_name in HIGH_INIT_FILES[:10]:  # Show first 10
    result = subprocess.run(
        ['grep', '-r', cpp_name, WORKSPACE / 'work_list.txt'],
        capture_output=True, text=True
    )
    if result.returncode != 0:
        continue
    
    line = result.stdout.strip()
    parts = line.split('|')
    if len(parts) < 2:
        continue
    
    cpp_rel = parts[0].strip()
    rust_rel = parts[1].strip()
    cpp_path = WORKSPACE / cpp_rel
    rust_path = WORKSPACE / rust_rel
    
    cpp_has_init = False
    rust_has_init = False
    
    if cpp_path.exists():
        cpp_content = cpp_path.read_text(encoding='utf-8', errors='ignore')
    if cpp_path.exists():
        cpp_content = cpp_path.read_text(encoding='utf-8', errors='ignore')
        # Look for void ClassName::init(params) pattern
        matches = re.findall(r'(\w+)\s*::\s*init\s*\([^)]*\)\s*{', cpp_content)
        cpp_has_init = len(matches) > 0
        cpp_init_sigs = matches[:3] if matches else []
    else:
        cpp_has_init = False
        cpp_init_sigs = []
    
    if rust_path.exists():
        rust_content = rust_path.read_text(encoding='utf-8', errors='ignore')
        # Look for fn init(...) -> 
        matches2 = re.findall(r'fn\s+init\s*\([^)]*\)\s*(?:->\s*[^{]+)?\s*{', rust_content)
        rust_has_init = len(matches2) > 0
        rust_init_sigs = matches2[:3] if matches2 else []
    else:
        rust_has_init = False
        rust_init_sigs = []
    
    if cpp_has_init != rust_has_init:
        print(f"\n{cpp_name}:")
        print(f"  C++ has init: {cpp_has_init} - signatures: {cpp_init_sigs if cpp_has_init else 'N/A'}")
        print(f"  Rust has init: {rust_has_init} - signatures: {rust_init_sigs if rust_has_init else 'N/A'}")
        print(f"  ⚠️  MISMATCH: {'Rust missing init' if not rust_has_init else 'C++ missing init'}")
    # else:
    #    print(f"✓ {cpp_name}: both have init")

print("\n\nNote: Some Rust files implement SubsystemInterface trait, which requires")
print("`fn init(&mut self)`. These may be defined in separate impl blocks.")
print("The simple regex check may miss them if they're in other files.")
