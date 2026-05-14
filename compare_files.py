#!/usr/bin/env python3
import sys
import re
from pathlib import Path

def extract_key_elements(filepath, language):
    """Extract key structural elements from a source file."""
    with open(filepath, 'r', encoding='utf-8', errors='ignore') as f:
        content = f.read()
    
    lines = content.split('
')
    result = {
        'total_lines': len(lines),
        'functions': [],
        'structs': [],
        'enums': [],
        'trait_impls': [],
        'mod_blocks': [],
    }
    
    if language == 'cpp':
        # Extract function signatures (rough)
        func_pattern = r'^\s*(?:virtual\s+)?(?:inline\s+)?(?:static\s+)?(?:[\w:*&<>]+)\s+(\w+)\s*\([^;]*\)\s*(?:const)?\s*(?:final)?\s*(?:override)?\s*\{?'
        struct_pattern = r'^\s*(?:class|struct)\s+(\w+)'
        enum_pattern = r'^\s*enum\s+(?:class\s+)?(\w+)'
        
        for i, line in enumerate(lines, 1):
            # Function match
            func_match = re.search(func_pattern, line)
            if func_match:
                result['functions'].append({
                    'name': func_match.group(1).strip(),
                    'line': i,
                    'signature': line.strip()[:100]
                })
            
            struct_match = re.search(struct_pattern, line)
            if struct_match:
                result['structs'].append({
                    'name': struct_match.group(1).strip(),
                    'line': i,
                    'signature': line.strip()[:100]
                })
            
            enum_match = re.search(enum_pattern, line)
            if enum_match:
                result['enums'].append({
                    'name': enum_match.group(1).strip(),
                    'line': i,
                })
    else:  # rust
        func_pattern = r'^\s*(?:pub\s+)?fn\s+(\w+)\s*\([^)]*\)\s*(?:->\s*[\w<>]+\s*)?\{'
        struct_pattern = r'^\s*(?:pub\s+)?struct\s+(\w+)'
        enum_pattern = r'^\s*(?:pub\s+)?enum\s+(\w+)'
        trait_pattern = r'^\s*(?:pub\s+)?trait\s+(\w+)'
        impl_pattern = r'^\s*impl(?:\s+(\w+))?'
        
        for i, line in enumerate(lines, 1):
            func_match = re.search(func_pattern, line)
            if func_match:
                result['functions'].append({
                    'name': func_match.group(1).strip(),
                    'line': i,
                    'signature': line.strip()[:100]
                })
            
            struct_match = re.search(struct_pattern, line)
            if struct_match:
                result['structs'].append({
                    'name': struct_match.group(1).strip(),
                    'line': i,
                    'signature': line.strip()[:100]
                })
            
            enum_match = re.search(enum_pattern, line)
            if enum_match:
                result['enums'].append({
                    'name': enum_match.group(1).strip(),
                    'line': i,
                })
            
            trait_match = re.search(trait_pattern, line)
            if trait_match:
                result['structs'].append({
                    'name': trait_match.group(1).strip(),
                    'line': i,
                    'kind': 'trait',
                    'signature': line.strip()[:100]
                })
            
            impl_match = re.search(impl_pattern, line)
            if impl_match:
                result['trait_impls'].append({
                    'target': impl_match.group(1).strip() if impl_match.group(1) else ' (self)',
                    'line': i,
                })
    
    return result

def compare_pair(cpp_path, rust_path):
    """Compare a C++ and Rust file pair."""
    cpp_data = extract_key_elements(cpp_path, 'cpp')
    rust_data = extract_key_elements(rust_path, 'rust')
    
    # Compare
    comparison = {
        'cpp_path': str(cpp_path),
        'rust_path': str(rust_path),
        'cpp_lines': cpp_data['total_lines'],
        'rust_lines': rust_data['total_lines'],
        'line_ratio': rust_data['total_lines'] / cpp_data['total_lines'] if cpp_data['total_lines'] > 0 else 0,
        'cpp_functions': len(cpp_data['functions']),
        'rust_functions': len(rust_data['functions']),
        'cpp_structs': len(cpp_data['structs']),
        'rust_structs': len(rust_data['structs']),
        'structural_match': None,
        'missing_rust_functions': [],
        'extra_rust_functions': [],
        'missing_cpp_structs': [],
        'extra_rust_structs': [],
        'notes': []
    }
    
    # Check for likely mismatches
    if abs(rust_data['total_lines'] - cpp_data['total_lines']) > cpp_data['total_lines'] * 0.5:
        comparison['notes'].append('Significant line count difference')
    
    # Compare function names (normalized)
    cpp_funcs = set(f['name'] for f in cpp_data['functions'] if f['name'])
    rust_funcs = set(f['name'] for f in rust_data['functions'] if f['name'])
    
    # Common C++ -> Rust transformations: init -> new, Update -> update, etc.
    def normalize_name(name):
        name = name.lower()
        if name.startswith('the'):
            name = name[3:]
        if name.endswith('impl'):
            name = name[:-4]
        return name
    
    cpp_normalized = set(normalize_name(n) for n in cpp_funcs)
    rust_normalized = set(normalize_name(n) for n in rust_funcs)
    
    # Find mismatches
    missing = cpp_normalized - rust_normalized
    extra = rust_normalized - cpp_normalized
    
    # Filter out likely noise
    def is_meaningful(name):
        noise = ['init', 'new', 'drop', 'default', 'clone', 'from', 'into', 'as_']
        return len(name) > 3 and name not in noise
    
    meaningful_missing = [n for n in missing if is_meaningful(n)]
    meaningful_extra = [n for n in extra if is_meaningful(n)]
    
    if meaningful_missing:
        comparison['missing_rust_functions'] = list(meaningful_missing)
        comparison['notes'].append(f"Missing {len(meaningful_missing)} meaningful functions")
    
    if meaningful_extra:
        comparison['extra_rust_functions'] = list(meaningful_extra)
        comparison['notes'].append(f"Extra {len(meaningful_extra)} Rust-only functions")
    
    # Compare structs
    cpp_structs = set(s['name'] for s in cpp_data['structs'] if s.get('kind', 'struct') == 'struct')
    rust_structs = set(s['name'] for s in rust_data['structs'] if s.get('kind', 'struct') == 'struct')
    
    cpp_struct_norm = set(normalize_name(n) for n in cpp_structs)
    rust_struct_norm = set(normalize_name(n) for n in rust_structs)
    
    missing_structs = cpp_struct_norm - rust_struct_norm
    extra_structs = rust_struct_norm - cpp_struct_norm
    
    if missing_structs:
        comparison['missing_cpp_structs'] = list(missing_structs)
        comparison['notes'].append(f"Missing {len(missing_structs)} C++ structs")
    
    if extra_structs:
        comparison['extra_rust_structs'] = list(extra_structs)
        comparison['notes'].append(f"Extra {len(extra_structs)} Rust-only structs")
    
    # Overall structural match assessment
    func_match_ratio = len(cpp_normalized & rust_normalized) / len(cpp_normalized) if cpp_normalized else 1.0
    struct_match_ratio = len(cpp_struct_norm & rust_struct_norm) / len(cpp_struct_norm) if cpp_struct_norm else 1.0
    
    if func_match_ratio >= 0.9 and struct_match_ratio >= 0.9:
        comparison['structural_match'] = 'high'
    elif func_match_ratio >= 0.7 or struct_match_ratio >= 0.7:
        comparison['structural_match'] = 'partial'
    else:
        comparison['structural_match'] = 'low'
    
    return comparison

if __name__ == '__main__':
    # Read file pairs from stdin (one per line: cpp_path|rust_path)
    results = []
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        parts = line.split('|')
        if len(parts) == 2:
            cpp_path, rust_path = parts
            try:
                comp = compare_pair(cpp_path, rust_path)
                results.append(comp)
            except Exception as e:
                results.append({
                    'cpp_path': cpp_path,
                    'rust_path': rust_path,
                    'error': str(e)
                })
    
    # Output results as JSON
    print(json.dumps(results, indent=2))
