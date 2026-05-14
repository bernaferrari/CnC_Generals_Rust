#!/usr/bin/env python3
"""
Comprehensive parity comparison between C++ GameClient and Rust GameClient.
Focus: initialization order, draw/render pipeline, UI state, input handling,
bridge to Main, message translators, asset loading, save/load integration, audio.
"""

import os
import subprocess
import re
from pathlib import Path
from collections import defaultdict

# Paths
CPP_ROOT = Path("GeneralsMD/Code/GameEngine/Source/GameClient")
RUST_ROOT = Path("GeneralsRust/Code/GameEngine/GameClient")

# Get all C++ .cpp files
cpp_files = sorted(CPP_ROOT.rglob("*.cpp"))

# Build mapping from C++ base name to Rust file(s)
rust_files = list(RUST_ROOT.rglob("*.rs"))
rust_map = defaultdict(list)
for rf in rust_files:
    # Skip examples, benches, tests
    if any(part in ["examples", "benches", "tests"] for part in rf.parts):
        continue
    # Skip gui/src/lib.rs duplicate entries
    if rf.name == "lib.rs":
        continue
    base = rf.stem
    rust_map[base].append(rf)

# Manually map important files
SPECIAL_MAPS = {
    "Display": ["src/render_bridge.rs", "src/display.rs"] if 0 else ["src/display_string_manager.rs"], # Need better mapping
    "GameClient": ["src/lib.rs", "src/game_client.rs"] if 0 else ["src/lib.rs"],
    "DrawableManager": ["src/drawable_manager.rs"],
    "DisplayStringManager": ["src/display_string_manager.rs"],
    "GameWindowManager": ["src/gui/game_window_manager.rs"],
    "GameFont": ["src/gui/game_font.rs"],
    "LoadScreen": ["src/gui/load_screen.rs"],
    "InGameUI": ["src/in_game_ui.rs", "src/gui/ingame_ui.rs"],
    "Input/Keyboard": ["src/input/keyboard.rs"],
    "Input/Mouse": ["src/input/mouse.rs"],
    "Eva": ["src/audio/audio_engine.rs", "src/audio/speech_system.rs"],
    "MessageStream/CommandXlat": ["src/message_stream/command_xlat.rs"],
    "MessageStream/GUICommandTranslator": ["src/message_stream/gui_command_translator.rs"],
    "MessageStream/SelectionXlat": ["src/message_stream/selection_xlat.rs"],
    "MessageStream/PlaceEventTranslator": ["src/message_stream/place_event_translator.rs"],
    "MessageStream/WindowXlat": ["src/message_stream/window_xlat.rs"],
    "MessageStream/LookAtXlat": ["src/message_stream/look_at_xlat.rs"],
    "MessageStream/HotKey": ["src/message_stream/hot_key.rs"],
    "GameClientDispatch": ["src/message_stream/message_stream.rs"],
    "GUI/ControlBar/ControlBar": ["src/gui/control_bar/control_bar.rs"],
    "GUI/AnimateWindowManager": ["src/gui/animate_window_manager.rs"],
    "GUI/Shell/Shell": ["src/gui/shell/shell.rs"],
    "VideoPlayer": ["src/video_player.rs"],
    "VideoStream": ["src/video_stream.rs"],
    "SelectionInfo": ["src/selection_info.rs"],
    "Statistics": ["src/statistics.rs"],
    "LanguageFilter": ["src/language_filter.rs"],
    "Line2D": ["src/line2_d.rs"],
    "MapUtil": ["src/map_util.rs"],
    "Color": ["src/color.rs", "src/display/color.rs"],
    "Snow": ["src/snow.rs"],
    "Water": ["src/water.rs", "src/terrain/water.rs"],
    "Terrain/TerrainVisual": ["src/terrain/terrain_visual.rs"],
    "Terrain/TerrainRoads": ["src/terrain/terrain_roads.rs"],
    "RadiusDecal": ["src/radius_decal.rs"],
    "ParabolicEase": ["src/parabolic_ease.rs"],
    "Radar": ["src/system/radar.rs"],
    "FXList": ["src/fx_list.rs"],
    "GameText": ["src/game_text.rs"],
    "GlobalLanguage": ["src/assets/localization.rs"],
    "GraphDraw": ["src/system/graph_draw.rs"],
    "System/Anim2D": ["src/system/anim2_d.rs"],
    "System/ParticleSys": ["src/system/particle_sys.rs"],
    "System/Smudge": ["src/system/smudge.rs"],
    "System/CampaignManager": ["src/system/campaign_manager.rs"],
    "System/Image": ["src/system/image.rs"],
    "Drawable/Update/BeaconClientUpdate": ["src/system/beacon_display.rs"],
}

def find_counterpart(cpp_file):
    """Find the Rust counterpart for a C++ file."""
    rel = cpp_file.relative_to(CPP_ROOT)
    parts = list(rel.parts)
    base = cpp_file.stem

    # Check special maps first
    key = "/".join(parts[1:-1] + [base]) if len(parts) > 1 else base
    # Try exact match
    for k, v in SPECIAL_MAPS.items():
        if k == key or k == base:
            return v[0] if v else None

    # Try generic mapping
    if base in rust_map:
        candidates = rust_map[base]
        # Filter out pre-compiled artifacts
        candidates = [c for c in candidates if "target/" not in str(c) and "Cargo.lock" not in str(c)]
        if candidates:
            return candidates[0]
    return None

# Parse C++ file to extract key functions, classes, and their line numbers
def parse_cpp_file(filepath):
    with open(filepath, 'r', encoding='utf-8', errors='ignore') as f:
        lines = f.readlines()

    functions = []
    classes = []
    # Simple patterns - can be improved
    func_pattern = re.compile(r'^\s*(?:inline\s+)?(?:static\s+)?[\w:\<\>\s\*&]+\s+([\w:]+)\s*\([^)]*\)\s*(?:const)?\s*(?:override)?\s*(?:final)?\s*(?:noexcept)?\s*\{?$')
    class_pattern = re.compile(r'^\s*(?:class|struct)\s+(\w+)')

    for i, line in enumerate(lines, 1):
        # Check for class declaration
        class_match = class_pattern.match(line)
        if class_match:
            classes.append((i, class_match.group(1)))

        # Skip lines that are clearly not function definitions
        stripped = line.strip()
        if stripped.startswith('//') or stripped.startswith('*') or stripped.endswith(';') or stripped.endswith(')') == False:
            if '(' not in line or ')' not in line:
                continue
        if '{' not in line and ';' not in stripped:
            continue
        if stripped in ['private:', 'public:', 'protected:']:
            continue

        func_match = func_pattern.match(line)
        if func_match:
            func_name = func_match.group(1)
            functions.append((i, func_name, stripped[:80]))

    return {
        'total_lines': len(lines),
        'classes': classes,
        'functions': functions,
    }

# Simple Rust parser
def parse_rust_file(filepath):
    try:
        with open(filepath, 'r', encoding='utf-8') as f:
            content = f.read()
        lines = content.split('\n')
    except Exception as e:
        return {'total_lines': 0, 'functions': [], 'error': str(e)}

    functions = []
    structs = []
    impls = []

    # Patterns for Rust
    struct_pattern = re.compile(r'^\s*(?:pub\s+)?struct\s+(\w+)')
    fn_pattern = re.compile(r'^\s*(?:pub\s+)?(?:async\s+)?fn\s+(\w+)\s*(?:<[^>]+>)?\s*\([^)]*\)\s*(?:->\s*[^{]+)?\s*\{?$')
    impl_pattern = re.compile(r'^\s*impl\s*(?:<[^>]+>\s*)?(\w+)')

    for i, line in enumerate(lines, 1):
        struct_match = struct_pattern.match(line)
        if struct_match:
            structs.append((i, struct_match.group(1)))

        impl_match = impl_pattern.match(line)
        if impl_match:
            impls.append((i, impl_match.group(1)))

        fn_match = fn_pattern.match(line)
        if fn_match:
            fn_name = fn_match.group(1)
            functions.append((i, fn_name, line.strip()[:80]))

    return {
        'total_lines': len(lines),
        'functions': functions,
        'structs': structs,
        'impls': impls,
    }

# Main comparison
print("=" * 120)
print("GAMECLIENT PARITY ANALYSIS")
print("=" * 120)
print()

# Focus on critical files for parity report
CRITICAL_CPP = [
    "GameClient.cpp",
    "GameClientDispatch.cpp",
    "Display.cpp",
    "DisplayString.cpp",
    "DisplayStringManager.cpp",
    "Drawable.cpp",
    "DrawableManager.cpp",
    "InGameUI.cpp",
    "Input/Keyboard.cpp",
    "Input/Mouse.cpp",
    "GUI/GameWindowManager.cpp",
    "GUI/GameWindow.cpp",
    "GUI/LoadScreen.cpp",
    "GUI/ControlBar/ControlBar.cpp",
    "GUI/Shell/Shell.cpp",
    "MessageStream/CommandXlat.cpp",
    "MessageStream/GUICommandTranslator.cpp",
    "MessageStream/SelectionXlat.cpp",
    "Audio/Eva.cpp",
    "System/CampaignManager.cpp",
    "VideoPlayer.cpp",
]

results = []

for cpp_rel in CRITICAL_CPP:
    cpp_path = CPP_ROOT / cpp_rel
    if not cpp_path.exists():
        print(f"WARNING: {cpp_rel} not found in C++")
        continue

    cpp_data = parse_cpp_file(cpp_path)
    rust_path = find_counterpart(cpp_path)

    if rust_path is None:
        status = "MISSING"
        rust_rel = "NOT FOUND"
    else:
        rust_rel_path = Path(rust_path) if isinstance(rust_path, str) else rust_path
        rust_rel = rust_rel_path.relative_to(RUST_ROOT.parent.parent.parent.parent)
        rust_data = parse_rust_file(rust_path)

        # Simple heuristic: compare function count and names
        cpp_funcs = set(name for _, name, _ in cpp_data['functions'])
        rust_funcs = set(name for _, name, _ in rust_data['functions'])

        missing_funcs = cpp_funcs - rust_funcs
        extra_funcs = rust_funcs - cpp_funcs

        if missing_funcs or extra_funcs:
            status = "DIFFERENT"
        else:
            status = "OK"

    results.append({
        'cpp': cpp_rel,
        'rust': str(rust_rel) if rust_path else "NA",
        'status': status,
        'cpp_funcs': len(cpp_data['functions']),
        'rust_funcs': len(rust_data['functions']) if rust_path else 0,
        'missing': missing_funcs if 'missing_funcs' in locals() else set(),
        'extra': extra_funcs if 'extra_funcs' in locals() else set(),
    })

# Print summary table
print(f"{'C++ File':<50} {'Rust File':<60} {'Status':<12} {'C++ Fns':>8} {'Rust Fns':>8} {'Diff'}")
print("-" * 180)
for r in results:
    diff = ""
    if r['missing'] or r['extra']:
        diff = f"Missing: {len(r['missing'])}, Extra: {len(r['extra'])}"
    print(f"{r['cpp']:<50} {r['rust']:<60} {r['status']:<12} {r['cpp_funcs']:>8} {r['rust_funcs']:>8} {diff}")

print()
print("\nDETAILED DIFFERENCES:")
print("=" * 120)

for r in results:
    if r['status'] != "OK":
        print(f"\n{r['cpp']} -> {r['rust']}")
        if r['missing']:
            print(f"  Missing C++ functions: {sorted(r['missing'])[:10]}...")
        if r['extra']:
            print(f"  Extra Rust functions: {sorted(r['extra'])[:10]}...")
