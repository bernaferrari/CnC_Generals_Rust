
import os
import re

def to_snake_case(name):
    s1 = re.sub('(.)([A-Z][a-z]+)', r'\1_\2', name)
    return re.sub('([a-z0-9])([A-Z])', r'\1_\2', s1).lower()

with open('cpp_files.txt', 'r') as f:
    cpp_files = [l.strip() for l in f.readlines()]

with open('rust_files.txt', 'r') as f:
    rust_files = [l.strip() for l in f.readlines()]

# Map Rust files by their filename (without extension) for quick lookup
rust_map = {}
for rf in rust_files:
    basename = os.path.basename(rf)
    name_no_ext = os.path.splitext(basename)[0]
    rust_map[name_no_ext] = rf

report = []
report.append("# Codebase Comparison Report")
report.append("This report compares the file structure of `GeneralsMD` (C++) and `GeneralsRust`.")
report.append("")

categories = {
    "Common": "Code/GameEngine/Source/Common",
    "GameLogic": "Code/GameEngine/Source/GameLogic",
    "GameClient": "Code/GameEngine/Source/GameClient",
    "GameEngineDevice": "Code/GameEngineDevice",
    "Libraries": "Code/Libraries"
}

missing_count = 0
total_cpp = 0

for category, prefix in categories.items():
    report.append(f"## {category} Analysis")
    report.append("| C++ File | Rust Equivalent | Status |")
    report.append("| --- | --- | --- |")
    
    category_cpp = [f for f in cpp_files if prefix in f]
    total_cpp += len(category_cpp)
    
    for cpp_f in category_cpp:
        basename = os.path.basename(cpp_f)
        name_no_ext = os.path.splitext(basename)[0]
        snake_name = to_snake_case(name_no_ext)
        
        # Try finding exact match or snake_case match
        match = rust_map.get(snake_name)
        if not match:
             match = rust_map.get(name_no_ext.lower())
        
        if match:
            # Check if it makes sense location-wise (loose check)
            # e.g. if cpp is in Common, rust should be in Common
            status = "✅ Found"
            if category.lower() not in match.lower() and "game_engine_device" not in match.lower(): 
                 status = "⚠️ Found (Different Module?)"
            
            report.append(f"| `{basename}` | `{os.path.basename(match)}` | {status} |")
        else:
            report.append(f"| `{basename}` | ❌ Missing | **MISSING** |")
            missing_count += 1
            
    report.append("")

report.append(f"## Summary")
report.append(f"- Total C++ Files Scanned: {total_cpp}")
report.append(f"- Missing Rust Implementations: {missing_count}")
report.append(f"- Coverage: {((total_cpp - missing_count) / total_cpp) * 100:.1f}%")

with open('CODEBASE_COMPARISON_REPORT.md', 'w') as f:
    f.write('\n'.join(report))

print(f"Report generated. Missing: {missing_count}")
