#!/usr/bin/env python3
"""Execute original C++ and production Rust repeat bodies on identical states.

Checks arbitration, unsigned frame arithmetic and sequence updates; bypasses
OS event ingestion, layouts and message delivery, so proves none of those.
DirectInput numeric fixture values are from Microsoft windows-sys 0.61.2
Win32/Devices/HumanInterfaceDevice. The entire key/layout pipeline is excluded.
Requires clang++ and rustc. Build products live in a temporary directory.
"""

from __future__ import annotations
import argparse
import itertools
import re
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
CPP = "GeneralsMD/Code/GameEngine/Source/GameClient/Input/Keyboard.cpp"
RUST = "GeneralsRust/Code/GameEngine/GameClient/src/input/keyboard.rs"
# Rust variant, SDK symbol, original numeric device code (independent fixture).
CODES = [
    ["F1", "DIK_F1", 59],
    ["F2", "DIK_F2", 60],
    ["F3", "DIK_F3", 61],
    ["F4", "DIK_F4", 62],
    ["F5", "DIK_F5", 63],
    ["F6", "DIK_F6", 64],
    ["F7", "DIK_F7", 65],
    ["F8", "DIK_F8", 66],
    ["F9", "DIK_F9", 67],
    ["F10", "DIK_F10", 68],
    ["F11", "DIK_F11", 87],
    ["F12", "DIK_F12", 88],
    ["Num1", "DIK_1", 2],
    ["Num2", "DIK_2", 3],
    ["Num3", "DIK_3", 4],
    ["Num4", "DIK_4", 5],
    ["Num5", "DIK_5", 6],
    ["Num6", "DIK_6", 7],
    ["Num7", "DIK_7", 8],
    ["Num8", "DIK_8", 9],
    ["Num9", "DIK_9", 10],
    ["Num0", "DIK_0", 11],
    ["A", "DIK_A", 30],
    ["B", "DIK_B", 48],
    ["C", "DIK_C", 46],
    ["D", "DIK_D", 32],
    ["E", "DIK_E", 18],
    ["F", "DIK_F", 33],
    ["G", "DIK_G", 34],
    ["H", "DIK_H", 35],
    ["I", "DIK_I", 23],
    ["J", "DIK_J", 36],
    ["K", "DIK_K", 37],
    ["L", "DIK_L", 38],
    ["M", "DIK_M", 50],
    ["N", "DIK_N", 49],
    ["O", "DIK_O", 24],
    ["P", "DIK_P", 25],
    ["Q", "DIK_Q", 16],
    ["R", "DIK_R", 19],
    ["S", "DIK_S", 31],
    ["T", "DIK_T", 20],
    ["U", "DIK_U", 22],
    ["V", "DIK_V", 47],
    ["W", "DIK_W", 17],
    ["X", "DIK_X", 45],
    ["Y", "DIK_Y", 21],
    ["Z", "DIK_Z", 44],
    ["Left", "DIK_LEFT", 203],
    ["Right", "DIK_RIGHT", 205],
    ["Up", "DIK_UP", 200],
    ["Down", "DIK_DOWN", 208],
    ["Home", "DIK_HOME", 199],
    ["End", "DIK_END", 207],
    ["PageUp", "DIK_PRIOR", 201],
    ["PageDown", "DIK_NEXT", 209],
    ["Space", "DIK_SPACE", 57],
    ["Enter", "DIK_RETURN", 28],
    ["Tab", "DIK_TAB", 15],
    ["Backspace", "DIK_BACK", 14],
    ["Delete", "DIK_DELETE", 211],
    ["Insert", "DIK_INSERT", 210],
    ["Escape", "DIK_ESCAPE", 1],
    ["Pause", "DIK_PAUSE", 197],
    ["PrintScreen", "DIK_SYSRQ", 183],
    ["LeftShift", "DIK_LSHIFT", 42],
    ["RightShift", "DIK_RSHIFT", 54],
    ["LeftCtrl", "DIK_LCONTROL", 29],
    ["RightCtrl", "DIK_RCONTROL", 157],
    ["LeftAlt", "DIK_LMENU", 56],
    ["RightAlt", "DIK_RMENU", 184],
    ["LeftMeta", "DIK_LWIN", 219],
    ["RightMeta", "DIK_RWIN", 220],
    ["NumPad0", "DIK_NUMPAD0", 82],
    ["NumPad1", "DIK_NUMPAD1", 79],
    ["NumPad2", "DIK_NUMPAD2", 80],
    ["NumPad3", "DIK_NUMPAD3", 81],
    ["NumPad4", "DIK_NUMPAD4", 75],
    ["NumPad5", "DIK_NUMPAD5", 76],
    ["NumPad6", "DIK_NUMPAD6", 77],
    ["NumPad7", "DIK_NUMPAD7", 71],
    ["NumPad8", "DIK_NUMPAD8", 72],
    ["NumPad9", "DIK_NUMPAD9", 73],
    ["NumPadAdd", "DIK_ADD", 78],
    ["NumPadSubtract", "DIK_SUBTRACT", 74],
    ["NumPadMultiply", "DIK_MULTIPLY", 55],
    ["NumPadDivide", "DIK_DIVIDE", 181],
    ["NumPadDecimal", "DIK_DECIMAL", 83],
    ["NumPadEnter", "DIK_NUMPADENTER", 156],
    ["CapsLock", "DIK_CAPITAL", 58],
    ["NumLock", "DIK_NUMLOCK", 69],
    ["ScrollLock", "DIK_SCROLL", 70],
    ["Minus", "DIK_MINUS", 12],
    ["Plus", "DIK_EQUALS", 13],
    ["LeftBracket", "DIK_LBRACKET", 26],
    ["RightBracket", "DIK_RBRACKET", 27],
    ["Semicolon", "DIK_SEMICOLON", 39],
    ["Quote", "DIK_APOSTROPHE", 40],
    ["Grave", "DIK_GRAVE", 41],
    ["Backslash", "DIK_BACKSLASH", 43],
    ["Slash", "DIK_SLASH", 53],
    ["Comma", "DIK_COMMA", 51],
    ["Period", "DIK_PERIOD", 52],
]


def sources(root: Path) -> tuple[str, str]:
    cpp = (root / CPP).read_text(encoding="latin-1")
    cpp = cpp[cpp.index("Bool Keyboard::checkKeyRepeat(") :]
    cpp = cpp[: cpp.index("}  // end checkKeyRepeat") + 1]
    rust = (root / RUST).read_text()
    enum = rust[
        rust.index("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]") : rust.index(
            "impl From<WinitKeyCode>"
        )
    ]
    repeat = rust[rust.index("    pub fn update_repeat(") :]
    repeat = repeat[: repeat.index("    /// Update state for next frame")]
    return cpp, enum + RUST_STATE + repeat + "}\n" + RUST_MAIN


CPP_PREFIX = r"""
#include <cstdint>
#include <iostream>
using Bool=bool; using Int=int; using UnsignedByte=uint8_t;
constexpr bool FALSE=false, TRUE=true;
constexpr int KEY_NONE=0, KEY_STATE_DOWN=2, KEY_STATE_AUTOREPEAT=256;
#define BitTest(x, flag) ((x)&(flag))
struct KeyboardIO { enum { STATUS_UNUSED=0 }; uint8_t key=0,status=0; uint16_t state=0; uint32_t sequence=0; };
struct Keyboard {
    static constexpr int NUM_KEYS=256, KEY_REPEAT_DELAY=10;
    KeyboardIO m_keys[256]{}, m_keyStatus[256]{};
    uint32_t m_inputFrame=0;
    bool checkKeyRepeat();
};
"""
CPP_MAIN = r"""
int main() {
    uint32_t frame, sequence; int count, code;
    while (std::cin >> frame >> count) {
        Keyboard k; k.m_inputFrame=frame;
        int first=256;
        for(int i=0;i<count;++i) {
            std::cin >> code >> sequence;
            k.m_keyStatus[code].state=KEY_STATE_DOWN;
            k.m_keyStatus[code].sequence=sequence;
            if(code<first) first=code;
        }
        for(int tick=0;tick<16;++tick) {
            if(tick==8) k.m_keyStatus[first].state=0;
            if(tick==10) { k.m_keyStatus[first].state=KEY_STATE_DOWN; k.m_keyStatus[first].sequence=k.m_inputFrame; }
            ++k.m_inputFrame;
            k.m_keys[0].key=KEY_NONE;
            const bool repeated=k.checkKeyRepeat();
            std::cout << (repeated ? int(k.m_keys[0].key) : -1);
            for(int key=0;key<256;++key) if(k.m_keyStatus[key].state&KEY_STATE_DOWN)
                std::cout << ' ' << key << ':' << k.m_keyStatus[key].sequence;
            std::cout << '\n';
        }
    }
}
"""
RUST_STATE = r"""
use std::collections::HashMap;
use std::time::Instant;
struct KeyboardState { key_sequences:HashMap<KeyCode,u32>, input_frame:u32, repeat_delay_frames:u32, repeat_interval_frames:u32 }
impl KeyboardState {
    fn is_key_down(&self, key:KeyCode)->bool { self.key_sequences.contains_key(&key) }
"""
RUST_MAIN = r"""
fn main() {
    use std::io::{Read, Write};
    let mut input=String::new(); std::io::stdin().read_to_string(&mut input).unwrap();
    let mut input=input.split_whitespace();
    let output=std::io::stdout(); let mut out=std::io::BufWriter::new(output.lock());
    while let Some(frame)=input.next() {
        let mut k=KeyboardState{key_sequences:HashMap::new(), input_frame:frame.parse().unwrap(), repeat_delay_frames:10,repeat_interval_frames:2};
        let count:usize=input.next().unwrap().parse().unwrap();
        let mut first=255;
        for _ in 0..count {
            let code:u8=input.next().unwrap().parse().unwrap();
            let sequence=input.next().unwrap().parse().unwrap();
            k.key_sequences.insert(from_scan(code), sequence);
            first=first.min(code);
        }
        for tick in 0..16 {
            if tick==8 { k.key_sequences.remove(&from_scan(first)); }
            if tick==10 { k.key_sequences.insert(from_scan(first),k.input_frame); }
            let repeat=k.update_repeat(Instant::now());
            write!(out,"{}",repeat.first().map(|key|to_scan(*key) as i32).unwrap_or(-1)).unwrap();
            let mut states:Vec<_>=k.key_sequences.iter().map(|(key,seq)|(to_scan(*key),*seq)).collect();
            states.sort();
            for (key,seq) in states { write!(out," {}:{}",key,seq).unwrap(); }
            writeln!(out).unwrap();
        }
    }
}
"""


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    args = parser.parse_args()
    cpp, rust = sources(args.repo_root)
    to_scan = (
        "fn to_scan(key:KeyCode)->u8 { match key {"
        + "".join(f"KeyCode::{key}=>{code}," for key, _, code in CODES)
        + "KeyCode::Unknown=>0} }"
    )
    from_scan = (
        "fn from_scan(code:u8)->KeyCode { match code {"
        + "".join(f"{code}=>KeyCode::{key}," for key, _, code in CODES)
        + '_=>panic!("unknown fixture code")} }'
    )
    header = (
        args.repo_root / "GeneralsMD/Code/GameEngine/Include/GameClient/Keyboard.h"
    ).read_text()
    delay = int(re.search(r"KEY_REPEAT_DELAY\s*=\s*(\d+)", header).group(1))
    rust_source = (args.repo_root / RUST).read_text()
    rust_delay = int(
        re.search(r"pub const KEY_REPEAT_DELAY: u32 = (\d+)", rust_source).group(1)
    )
    rust_offset = int(
        re.search(r"pub const KEY_REPEAT_INTERVAL: u32 = (\d+)", rust_source).group(1)
    )
    cpp_prefix = CPP_PREFIX.replace("KEY_REPEAT_DELAY=10", f"KEY_REPEAT_DELAY={delay}")
    rust = rust.replace(
        "repeat_delay_frames:10,repeat_interval_frames:2",
        f"repeat_delay_frames:{rust_delay},repeat_interval_frames:{rust_offset}",
    )
    mapping_assertions = "".join(
        f"assert_eq!(KeyCode::{key}.repeat_scan_code(),Some({code}));"
        for key, _, code in CODES
    )
    rust = rust.replace("fn main() {", "fn main() {" + mapping_assertions, 1)
    codes = [code for _, _, code in CODES]
    cases = []
    for a, b in itertools.combinations(codes, 2):
        cases.append((10, [(a, 0), (b, 0)]))
        cases.append((10, [(a, 10), (b, 0)]))
    for code in codes:
        for frame, seq in [
            (0, 0),
            (9, 0),
            (10, 0),
            (0xFFFFFFFF, 0xFFFFFFF5),
            (0, 0xFFFFFFF6),
            (3, 0xFFFFFFF0),
        ]:
            cases.append((frame, [(code, seq)]))
    data = "".join(
        f"{frame} {len(keys)} " + " ".join(f"{key} {seq}" for key, seq in keys) + "\n"
        for frame, keys in cases
    )
    with tempfile.TemporaryDirectory(prefix="generals-repeat-parity-") as tmp:
        tmp = Path(tmp)
        (tmp / "original.cpp").write_text(cpp_prefix + cpp + CPP_MAIN)
        (tmp / "port.rs").write_text(rust + to_scan + from_scan)
        subprocess.run(
            [
                "clang++",
                "-std=c++17",
                str(tmp / "original.cpp"),
                "-o",
                str(tmp / "original"),
            ],
            check=True,
        )
        subprocess.run(
            [
                "rustc",
                "--edition=2024",
                "-Awarnings",
                str(tmp / "port.rs"),
                "-o",
                str(tmp / "port"),
            ],
            check=True,
        )
        original = subprocess.check_output(
            [str(tmp / "original")], input=data, text=True
        ).splitlines()
        port = subprocess.check_output(
            [str(tmp / "port")], input=data, text=True
        ).splitlines()
        if original != port:
            for index, (a, b) in enumerate(itertools.zip_longest(original, port)):
                if a != b:
                    raise SystemExit(
                        f"FAIL case={index // 16} tick={index % 16} fixture={cases[index // 16]} C++={a} Rust={b}"
                    )
    print(
        f"PASS {len(cases)} prepared-state cases, {len(original)} repeat frames; OS ingestion and message delivery not verified"
    )


if __name__ == "__main__":
    main()
