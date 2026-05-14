#!/usr/bin/env python3
import json

WORKSPACE = "/Users/bernardoferrari/Downloads/CnC_Generals_Zero_Hour-main"

with open(f"{WORKSPACE}/mismatch_report.json") as f:
    data = json.load(f)

print("=" * 80)
print("CRITICAL FINDINGS")
print("=" * 80)
critical = [m for m in data['mismatches'] if m['severity'] == 'critical']
for m in critical:
    cpp_file = m['cpp_path'].replace(WORKSPACE + '/', '')
    rust_file = m['rust_path'].replace(WORKSPACE + '/', '')
    print(f"\nFile: {cpp_file}")
    print(f"  Category: {m['category']}")
    print(f"  C++: {m['cpp_behavior']}")
    print(f"  Rust: {m['rust_deviation']}")
    print(f"  Fix: {m['suggested_fix']}")
    if 'singleton' in m.get('details', {}):
        print(f"  Singleton: {m['details']['singleton']} (cpp_has={m['details']['cpp_has']}, rust_has={m['details']['rust_has']})")

print(f"\n\n{'=' * 80}")
print("HIGH SEVERITY FINDINGS (Top 30)")
print("=" * 80)
high = [m for m in data['mismatches'] if m['severity'] == 'high']
for i, m in enumerate(high[:30], 1):
    cpp_file = m['cpp_path'].replace(WORKSPACE + '/', '')
    rust_file = m['rust_path'].replace(WORKSPACE + '/', '')
    print(f"\n#{i}: {cpp_file.split('/')[-1]}")
    print(f"  Category: {m['category']}")
    if m['category'] == 'initialization':
        print(f"  Issue: Missing init function in Rust")
    elif m['category'] == 'global_state':
        singleton = m['details'].get('singleton', 'unknown')
        print(f"  Issue: Global singleton '{singleton}' missing in Rust")
    elif m['category'] == 'api':
        print(f"  Function signature mismatch")
        print(f"  C++: {m['details'].get('cpp_sig','N/A')}")
        print(f"  Rust: {m['details'].get('rust_sig','N/A')}")

print(f"\n\nTotal critical: {len(critical)}")
print(f"Total high severity: {len(high)}")
print(f"Total medium: {len([m for m in data['mismatches'] if m['severity'] == 'medium'])}")
