#!/usr/bin/env python3
"""
Automated difference analyzer for C++/Rust file pairs.
Focuses on: initialization, state management, logic flow, API parity.
"""

import re
import os
import json
from pathlib import Path
from dataclasses import dataclass, asdict
from typing import List, Dict, Optional

WORKSPACE = Path("/Users/bernardoferrari/Downloads/CnC_Generals_Zero_Hour-main")

@dataclass
class MismatchReport:
    cpp_path: str
    rust_path: str
    category: str  # initialization, state, logic, api
    cpp_behavior: str
    rust_deviation: str
    severity: str  # critical, high, medium, low
    suggested_fix: str
    details: Dict

def extract_functions(content: str) -> Dict[str, str]:
    """Extract function signatures and bodies."""
    functions = {}
    # Match C++/Rust function patterns
    # C++: return_type function_name(params) { ... }
    # Rust: fn function_name(params) -> return_type { ... }
    patterns = [
        r'(\w[\w\s:*&]*\w)\s+(\w+)\s*\(([^)]*)\)\s*(?:->\s*([^{\s]+))?\s*\{',
        r'fn\s+(\w+)\s*\(([^)]*)\)\s*(?:->\s*([^{\s]+))?\s*\{',
    ]
    for pattern in patterns:
        for match in re.finditer(pattern, content, re.MULTILINE | re.DOTALL):
            if 'fn' in pattern:
                name = match.group(1)
                params = match.group(2)
                ret = match.group(3) or ''
                functions[name] = f"fn {name}({params}) -> {ret}"
            else:
                ret = match.group(1).strip()
                name = match.group(2)
                params = match.group(3)
                functions[name] = f"{ret} {name}({params})"
    return functions

def extract_state_variables(content: str) -> List[str]:
    """Extract member/static variable declarations."""
    vars = []
    # C++ member: type var_name;
    # Rust struct fields: var_name: type,
    patterns = [
        r'^\s*(\w[\w\s:*&]*\w)\s+(\w+)\s*[;{=]',
        r'^\s*(\w+)\s*:\s*([^,;\s]+)',
    ]
    for pattern in patterns:
        for match in re.finditer(pattern, content, re.MULTILINE):
            if 'fn' in pattern:
                continue
            if 'struct' in content[max(0, match.start()-50):match.start()]:
                continue
            if 'enum' in content[max(0, match.start()-50):match.start()]:
                continue
            if 'impl' in content[max(0, match.start()-50):match.start()]:
                continue
            if 'trait' in content[max(0, match.start()-50):match.start()]:
                continue
            vars.append(match.group(0).strip())
    return list(set(vars))

def check_global_singletons(cpp_content: str, rust_content: str) -> List[Dict]:
    """Check for global singleton patterns like TheGameLogic, TheAudio, etc."""
    issues = []
    singletons = ['TheGameLogic', 'TheAudio', 'TheMessageStream', 'TheGameClient', 'TheGameLogic']
    
    for singleton in singletons:
        cpp_pattern = f'(?:^|\s){singleton}\\s*(?:=|\\()'
        rust_pattern = f'(?:^|\s){singleton.lower()}\\s*::'
        
        cpp_has = bool(re.search(cpp_pattern, cpp_content))
        rust_has = bool(re.search(rust_pattern, rust_content))
        
        if cpp_has != rust_has:
            issues.append({
                'singleton': singleton,
                'cpp_has': cpp_has,
                'rust_has': rust_has,
                'message': f"Singleton '{singleton}' presence mismatch"
            })
    return issues

def analyze_init_sequence(cpp_path: Path, rust_path: Path) -> Optional[MismatchReport]:
    """Analyze initialization sequences."""
    try:
        cpp_content = cpp_path.read_text(encoding='utf-8', errors='ignore')
        rust_content = rust_path.read_text(encoding='utf-8', errors='ignore')
    except:
        return None
    
    report = None
    
    # Check for init function patterns
    cpp_has_init = 'init(' in cpp_content or 'Initialize(' in cpp_content
    rust_has_init = 'fn init' in rust_content or 'fn initialize' in rust_content
    
    if cpp_has_init != rust_has_init:
        report = MismatchReport(
            cpp_path=str(cpp_path),
            rust_path=str(rust_path),
            category='initialization',
            cpp_behavior='Has explicit init function' if cpp_has_init else 'No explicit init function',
            rust_deviation='Has explicit init function' if rust_has_init else 'No explicit init function',
            severity='high',
            suggested_fix='Ensure matching initialization API',
            details={'type': 'init_presence'}
        )
    
    # Check for constructor patterns
    cpp_ctors = re.findall(r'(\w+)::\1\s*\(', cpp_content)
    rust_ctors = re.findall(r'impl\s+(\w+)\s*\{', rust_content)
    
    return report

def analyze_file_pair(cpp_path: Path, rust_path: Path, priority: bool = False) -> List[MismatchReport]:
    """Analyze a single C++/Rust file pair for mismatches."""
    reports = []
    
    try:
        cpp_content = cpp_path.read_text(encoding='utf-8', errors='ignore')
        rust_content = rust_path.read_text(encoding='utf-8', errors='ignore')
    except Exception as e:
        return [MismatchReport(
            str(cpp_path), str(rust_path), 'system',
            'Could not read file', str(e), 'medium',
            'Check file encoding', {'error': str(e)}
        )]
    
    # 1. Check global singletons
    singleton_issues = check_global_singletons(cpp_content, rust_content)
    for issue in singleton_issues:
        reports.append(MismatchReport(
            str(cpp_path), str(rust_path), 'global_state',
            f"Found {issue['singleton']}",
            f"{'Missing' if not issue['rust_has'] else 'Present'} {issue['singleton']}",
            'critical' if issue['singleton'] in ['TheGameLogic', 'TheAudio'] else 'high',
            f"Add {issue['singleton']} global variable",
            issue
        ))
    
    # 2. Function signature parity
    cpp_funcs = extract_functions(cpp_content)
    rust_funcs = extract_functions(rust_content)
    
    common_funcs = set(cpp_funcs.keys()) & set(rust_funcs.keys())
    for func in common_funcs:
        if cpp_funcs[func] != rust_funcs[func]:
            reports.append(MismatchReport(
                str(cpp_path), str(rust_path), 'api',
                cpp_funcs[func],
                rust_funcs[func],
                'high',
                'Synchronize function signature',
                {'function': func, 'cpp_sig': cpp_funcs[func], 'rust_sig': rust_funcs[func]}
            ))
    
    # 3. State variables (simplified check)
    cpp_vars = extract_state_variables(cpp_content)
    rust_vars = extract_state_variables(rust_content)
    
    # Compare total counts as a heuristic
    if len(cpp_vars) > 0 and len(rust_vars) > 0:
        ratio = len(rust_vars) / len(cpp_vars) if len(cpp_vars) > 0 else 0
        if ratio < 0.5 or ratio > 1.5:
            reports.append(MismatchReport(
                str(cpp_path), str(rust_path), 'state',
                f"State variables: C++={len(cpp_vars)}",
                f"State variables: Rust={len(rust_vars)}",
                'medium',
                'Review state variable mapping',
                {'cpp_count': len(cpp_vars), 'rust_count': len(rust_vars)}
            ))
    
    # 4. Critical initialization patterns for priority files
    if priority:
        init_report = analyze_init_sequence(cpp_path, rust_path)
        if init_report:
            reports.append(init_report)
    
    return reports

def main():
    work_list_path = WORKSPACE / 'work_list.txt'
    lines = work_list_path.read_text().strip().split('\n')
    
    all_reports = []
    priority_keywords = [
        'GameEngine.cpp', 'game_engine.rs',
        'SubsystemInterface', 'subsystem_interface',
        'Snapshot', 'snapshot',
        'Xfer', 'xfer',
        'INI', 'ini',
        'ThingFactory', 'thing_factory',
        'MessageStream', 'message_stream',
        'Audio.cpp', 'game_audio.rs',
        'GameMain.cpp', 'game_main.rs',
        'GlobalData', 'global_data',
    ]
    
    print(f"Analyzing {len(lines)} file pairs...")
    
    for i, line in enumerate(lines):
        parts = line.split('|')
        if len(parts) != 2:
            continue
        
        cpp_rel, rust_rel = parts[0].strip(), parts[1].strip()
        
        # Construct absolute paths
        cpp_path = WORKSPACE / cpp_rel
        rust_path = WORKSPACE / rust_rel
        
        # Check if this is a priority file
        is_priority = any(kw in cpp_rel or kw in rust_rel for kw in priority_keywords)
        
        try:
            reports = analyze_file_pair(cpp_path, rust_path, priority=is_priority)
            all_reports.extend(reports)
            
            if (i+1) % 100 == 0:
                print(f"  Processed {i+1}/{len(lines)} pairs, found {len(all_reports)} issues so far")
        except Exception as e:
            print(f"Error processing pair {i+1}: {e}")
    
    # Categorize and summarize
    summary = {
        'total_pairs': len(lines),
        'total_mismatches': len(all_reports),
        'by_category': {},
        'by_severity': {},
        'mismatches': [asdict(r) for r in all_reports]
    }
    
    for report in all_reports:
        cat = report.category
        sev = report.severity
        summary['by_category'][cat] = summary['by_category'].get(cat, 0) + 1
        summary['by_severity'][sev] = summary['by_severity'].get(sev, 0) + 1
    
    # Write report
    report_path = WORKSPACE / 'mismatch_report.json'
    report_path.write_text(json.dumps(summary, indent=2))
    
    print(f"\nAnalysis complete!")
    print(f"Total pairs analyzed: {len(lines)}")
    print(f"Total mismatches found: {len(all_reports)}")
    print(f"Report saved to: {report_path}")
    print("\nSummary by Category:")
    for cat, count in summary['by_category'].items():
        print(f"  {cat}: {count}")
    print("\nSummary by Severity:")
    for sev, count in summary['by_severity'].items():
        print(f"  {sev}: {count}")

if __name__ == '__main__':
    main()
