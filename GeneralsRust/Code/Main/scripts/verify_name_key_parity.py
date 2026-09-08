#!/usr/bin/env python3
"""Compile original C++ and production Rust name-key state; compare operation traces.

The C++ scaffolding supplies only engine types/allocation macros. All hash,
lookup, insertion, reset and reverse-lookup bodies come from the source files.
Rust extraction excludes only the ambient singleton API, so each trace owns its
registry. This verifies ASCII behavior, not locale or instance-ownership parity.
"""
from pathlib import Path
import hashlib
import json
import random
import re
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[4]
CPP = ROOT / "GeneralsMD/Code/GameEngine/Source/Common/NameKeyGenerator.cpp"
RS = ROOT / "GeneralsRust/Code/GameEngine/Common/src/common/name_key_generator.rs"


def function(source: str, marker: str) -> str:
    start = source.index(marker)
    body = source.index("{", start)
    depth = 1
    end = body + 1
    while depth:
        depth += (source[end] == "{") - (source[end] == "}")
        end += 1
    return source[start:end]


def run(command, **kwargs):
    return subprocess.run(command, check=True, text=True, capture_output=True, **kwargs)


def main():
    cpp = CPP.read_text()
    rust = RS.read_text()
    cpp_bodies = cpp[cpp.index("NameKeyGenerator *TheNameKeyGenerator"):cpp.index("// Get a string out of the INI.")]
    rust_state = rust[rust.index("pub type NameKeyType"):rust.index("/// Shared state for the generator.")]
    rust_hash = function(rust, "fn calc_hash(")
    cpp_scaffold = r'''
#include <cstdint>
#include <cctype>
#include <cstring>
#include <strings.h>
#include <string>
#include <iostream>
using UnsignedInt = uint32_t;
using NameKeyType = uint32_t;
using Int = int;
using Byte = unsigned char;
constexpr NameKeyType NAMEKEY_INVALID = 0;
struct AsciiString : std::string {
    using std::string::string;
    using std::string::operator=;
    const char* str() const { return c_str(); }
    static const AsciiString TheEmptyString;
};
const AsciiString AsciiString::TheEmptyString;
#define DEBUG_ASSERTCRASH(...) ((void)0)
#define newInstance(T) new T
#define _stricmp strcasecmp
class NameKeyGenerator {
    struct Bucket {
        NameKeyType m_key;
        AsciiString m_nameString;
        Bucket* m_nextInSocket;
        void deleteInstance() { delete this; }
    };
    static constexpr int SOCKET_COUNT = 45007;
    UnsignedInt m_nextID;
    Bucket* m_sockets[SOCKET_COUNT];
public:
    NameKeyGenerator();
    ~NameKeyGenerator();
    void init();
    void reset();
    void freeSockets();
    AsciiString keyToName(NameKeyType);
    NameKeyType nameToKey(const char*);
    NameKeyType nameToLowercaseKey(const char*);
};
'''
    cpp_main = r'''
int main() {
    NameKeyGenerator state; state.init();
    for (std::string line; std::getline(std::cin, line);) {
        if (line == "R") { state.reset(); std::cout << "reset\n"; continue; }
        const char* name = line.c_str() + 2;
        auto key = line[0] == 'E' ? state.nameToKey(name) : state.nameToLowercaseKey(name);
        std::cout << key << ':' << state.keyToName(key) << '\n';
    }
}
'''
    rust_main = r'''
fn main() {
    use std::io::{self, BufRead};
    let mut state = NameKeyGeneratorState::new();
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        if line == "R" { state.reset(); println!("reset"); continue; }
        let name = &line[2..];
        let key = if line.starts_with('E') { state.name_to_key(name) } else { state.name_to_lowercase_key(name) };
        println!("{}:{}", key, state.key_to_name(key).unwrap_or_default());
    }
}
'''
    lower, variant = "aaaaaaaaaaaaaaa", "AAaAAaaAAAAAAAA"
    operations = [f"L {lower}", f"E {variant}", f"L {lower}", f"E {lower}", "R", f"E {variant}", f"L {lower}"]
    rng = random.Random(731)
    names = [lower, variant, "", "ControlBar.wnd:MoneyDisplay", "ActiveShroudUpgrade"]
    names += [f"Object_{index:04d}_ModuleTag" for index in range(100)]
    for _ in range(5000):
        if rng.randrange(101) == 0:
            operations.append("R")
            continue
        name = rng.choice(names)
        if rng.randrange(2):
            name = ''.join(ch.upper() if rng.randrange(2) else ch.lower() for ch in name)
        operations.append(rng.choice(["E ", "L "]) + name)
    trace = '\n'.join(operations) + '\n'
    with tempfile.TemporaryDirectory(prefix="generals-name-key-parity-") as directory:
        tmp = Path(directory)
        cpp_file, rust_file = tmp / "original.cpp", tmp / "production.rs"
        cpp_file.write_text(cpp_scaffold + cpp_bodies + cpp_main)
        rust_file.write_text("use std::collections::HashMap;\n" + rust_state + rust_hash + rust_main)
        run([shutil.which("clang++") or "c++", "-std=c++17", "-O0", str(cpp_file), "-o", str(tmp / "cpp")])
        run(["rustc", "--edition=2024", str(rust_file), "-o", str(tmp / "rust")])
        expected = run([str(tmp / "cpp")], input=trace).stdout
        actual = run([str(tmp / "rust")], input=trace).stdout
        if actual != expected:
            for index, (left, right) in enumerate(zip(expected.splitlines(), actual.splitlines())):
                if left != right:
                    raise AssertionError(f"operation {index} {operations[index]!r}: C++={left!r}, Rust={right!r}")
            raise AssertionError("trace output lengths differ")
        negative_state, replacements = re.subn(r"\.iter\(\)\s*\.rev\(\)", ".iter()", rust_state)
        assert replacements == 2, "negative control must restore both old lookup directions"
        rust_file.write_text("use std::collections::HashMap;\n" + negative_state + rust_hash + rust_main)
        run(["rustc", "--edition=2024", str(rust_file), "-o", str(tmp / "old-rust")])
        negative = run([str(tmp / "old-rust")], input=trace).stdout
        assert negative != expected, "old lookup order must fail the original trace"
        print(json.dumps({"operations": len(operations), "matched": True, "old_lookup_negative_control_failed": True,
            "cpp_sha256": hashlib.sha256(CPP.read_bytes()).hexdigest(),
            "rust_sha256": hashlib.sha256(RS.read_bytes()).hexdigest()}, indent=2))


if __name__ == "__main__":
    main()
