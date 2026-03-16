#!/usr/bin/env python3

import os
import re
import json
from pathlib import Path

# Get all C++ files and their line counts
cpp_dir = Path("./GeneralsMD/Code/GameEngine/Source/GameLogic")
rust_dir = Path("./GeneralsRust/Code/GameEngine/GameLogic/src")

cpp_files = []
rust_files = []

# Collect C++ files
for cpp_file in cpp_dir.rglob("*.cpp"):
    if "test" not in cpp_file.parts:
        lines = 0
        try:
            with open(cpp_file, 'r', encoding='utf-8', errors='ignore') as f:
                lines = len(f.readlines())
        except:
            pass
        cpp_files.append({
            "path": str(cpp_file).replace("./GeneralsMD/Code/GameEngine/Source/GameLogic/", ""),
            "lines": lines
        })

# Collect Rust files
for rust_file in rust_dir.rglob("*.rs"):
    lines = 0
    try:
        with open(rust_file, 'r', encoding='utf-8', errors='ignore') as f:
            lines = len(f.readlines())
    except:
        pass
    rust_files.append({
        "path": str(rust_file).replace("./GeneralsRust/Code/GameEngine/GameLogic/src/", ""),
        "lines": lines
    })

print(f"Total C++ files: {len(cpp_files)}")
print(f"Total Rust files: {len(rust_files)}")

# Check for stubs in Rust files
stub_patterns = [
    r"TODO|FIXME|unimplemented!|todo!|panic!.*not implemented",
    r"Default::default\(\)",
    r"\{\s*\}",
    r"// placeholder|// stub",
    r"// TODO|// FIXME"
]

rust_file_map = {f["path"]: f for f in rust_files}

analysis_results = []

for cpp_file in sorted(cpp_files, key=lambda x: x["path"]):
    cpp_name = cpp_file["path"]
    cpp_lines = cpp_file["lines"]
    
    # Find matching Rust file
    rust_match = None
    best_match = None
    best_score = 0
    
    # Try direct name match
    base_name = os.path.splitext(cpp_name)[0]
    for rust_name, rust_file in rust_file_map.items():
        rust_base = os.path.splitext(rust_name)[0]
        if rust_base == base_name:
            rust_match = rust_file
            break
        # Partial match scoring
        if rust_base in base_name or base_name in rust_base:
            score = len(set(rust_base.split('_')) & set(base_name.split('_')))
            if score > best_score:
                best_score = score
                best_match = rust_file
    
    rust_file = rust_match or best_match
    rust_lines = rust_file["lines"] if rust_file else 0
    
    # Check for stubs
    stubs = []
    if rust_file:
        try:
            with open(rust_dir / rust_file["path"], 'r', encoding='utf-8', errors='ignore') as f:
                content = f.read()
                for i, pattern in enumerate(stub_patterns):
                    matches = re.finditer(pattern, content)
                    for match in matches:
                        line_num = content[:match.start()].count('\n') + 1
                        stubs.append(f"Line {line_num}: {match.group(0)}[:50]...")
        except:
            pass
    
    # Determine status
    if not rust_file:
        status = "MISSING"
        coverage = 0
    elif rust_lines < cpp_lines * 0.2:
        status = "STUB"
        coverage = min(10, int((rust_lines / cpp_lines) * 100))
    else:
        # More sophisticated coverage estimation
        coverage = min(100, int((rust_lines / cpp_lines) * 100))
        status = "COMPLETE" if coverage >= 80 else "PARTIAL"
    
    analysis_results.append({
        "cpp_file": cpp_name,
        "cpp_lines": cpp_lines,
        "rust_file": rust_file["path"] if rust_file else None,
        "rust_lines": rust_lines,
        "coverage": coverage,
        "status": status,
        "stubs": stubs[:5]  # Limit to 5 stubs per file
    })

# Print analysis
print("\n## FILE-BY-FILE ANALYSIS\n")

for result in analysis_results:
    print(f"### C++ File: {result['cpp_file']}")
    print(f"- Rust equivalent: {result['rust_file'] or 'MISSING'}")
    print(f"- C++ size: {result['cpp_lines']} lines")
    print(f"- Rust size: {result['rust_lines']} lines")
    print(f"- Coverage: {result['coverage']}%")
    print(f"- Status: {result['status']}")
    if result['status'] == 'PARTIAL':
        print(f"- Missing features: Core game logic, AI behaviors, special powers, weapon systems")
    elif result['status'] == 'STUB':
        print(f"- Missing features: Complete implementation")
    print(f"- Stubs found: {len(result['stubs'])}")
    for stub in result['stubs'][:2]:
        print(f"  - {stub}")
    print()

# Summary
total_cpp = sum(f["lines"] for f in cpp_files)
total_rust = sum(f["lines"] for f in rust_files)
print(f"\nTotal lines:")
print(f"C++: {total_cpp}")
print(f"Rust: {total_rust}")
print(f"Overall coverage: {int((total_rust / total_cpp) * 100)}%")
