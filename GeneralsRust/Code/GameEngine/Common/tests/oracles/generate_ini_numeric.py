#!/usr/bin/env python3
"""Run extracted original INI methods against the original GameCommon header.

This is a host C++ arithmetic oracle, not a build of the original Windows game.
The INI token supplier is a stub; only complete finite decimal tokens are used.
No retail assets are needed. Run with --output pointing outside the source tree,
then compare ini_numeric_original.txt with the checked-in fixture.
"""

import argparse
import hashlib
from pathlib import Path
import re
import struct
import subprocess


PINS = {
    "GeneralsMD/Code/GameEngine/Source/Common/INI/INI.cpp":
        "a8aa77e908d2482aa774408be94cc9b7a1fa13fa4b48d0ea64311e85a9e3a5e7",
    "GeneralsMD/Code/GameEngine/Include/Common/GameCommon.h":
        "5765730dca375098a638184ce473cfbe43fed12623a1e8234ccd219883a5e54a",
    "GeneralsMD/Code/Libraries/Include/Lib/BaseType.h":
        "590af4e59d0e48510d976eeafa248000a1ac9f05d8f905f66f24b1c1cf9eceef",
}


def extract_method(source, name):
    match = re.search(r"(?:void|Real) INI::" + name + r"\(", source)
    assert match, name
    start = source.index("{", match.start())
    depth = 1
    end = start + 1
    while depth:
        depth += (source[end] == "{") - (source[end] == "}")
        end += 1
    return source[match.start():end]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[6])
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    sources = {}
    for relative, expected in PINS.items():
        data = (args.repo / relative).read_bytes()
        assert hashlib.sha256(data).hexdigest() == expected, relative
        sources[relative] = data.decode()
    ini_source, _, base_types = sources.values()
    shim = output / "include/Lib"
    shim.mkdir(parents=True, exist_ok=True)
    declarations = [re.search(r"^typedef[^;]+\b" + name + r";", base_types, re.M)[0]
                    for name in ("Real", "Int", "UnsignedInt", "UnsignedShort", "Bool")]
    pi = re.search(r"^#define PI\s+[^\n]+", base_types, re.M)[0]
    (shim / "BaseType.h").write_text("#pragma once\n" + pi + "\n" + "\n".join(declarations) + "\n")

    methods = ("parseReal", "parseAngleReal", "parseAngularVelocityReal",
               "parseVelocityReal", "parseAccelerationReal", "parseDurationReal")
    declarations = "\n".join(f"static void {name}(INI*, void*, void*, const void*);" for name in methods)
    extracted = "\n\n".join(extract_method(ini_source, name)
                            for name in ("scanReal", "scanPercentToReal", *methods))
    program = '''#include <cstdio>
#include <cstdint>
#include <cstring>
#include <iostream>
#include <string>
#include "Common/GameCommon.h"
static_assert(sizeof(Real) == 4 && sizeof(UnsignedInt) == 4);
enum { INI_INVALID_DATA = 1 };
class INI {
public:
    const char* token;
    const char* getNextToken() { return token; }
    static Real scanReal(const char*);
    static Real scanPercentToReal(const char*);
''' + declarations + "\n};\n" + extracted + '''
std::uint32_t bits(Real value) {
    std::uint32_t result;
    std::memcpy(&result, &value, sizeof(result));
    return result;
}
int main() {
    using Parse = void (*)(INI*, void*, void*, const void*);
    Parse methods[] = {INI::parseReal, INI::parseAngleReal,
        INI::parseAngularVelocityReal, INI::parseVelocityReal,
        INI::parseAccelerationReal, INI::parseDurationReal};
    std::string token;
    while (std::getline(std::cin, token)) {
        INI ini{token.c_str()};
        std::printf("%s", token.c_str());
        for (Parse method : methods) {
            Real result = 0;
            method(&ini, nullptr, &result, nullptr);
            std::printf(" %08x", bits(result));
        }
        std::printf(" %08x\\n", bits(INI::scanPercentToReal(token.c_str())));
    }
}
'''
    cpp = output / "ini_numeric_original.cpp"
    cpp.write_text(program)
    binary = output / "ini_numeric_original"
    command = ["c++", "-std=c++17", "-O0", "-ffp-contract=off", "-fno-fast-math",
               "-I" + str(output / "include"),
               "-I" + str(args.repo / "GeneralsMD/Code/GameEngine/Include"),
               str(cpp), "-o", str(binary)]
    subprocess.run(command, check=True)

    tokens = ["0", "-0", "0.1", "-0.1", "1", "-1", "10", "30", "45", "90",
              "180", "360", "900", "1000", "9.8", "-9.8", "999999", "+12.5"]
    # Adjacent values around exponent boundaries exercise normal/subnormal values,
    # underflow, ordinary magnitudes and near-maximum finite values, both signs.
    centers = [1, 0x00800000, 0x3f000000, 0x3f800000, 0x41f00000,
               0x42b40000, 0x447a0000, 0x7f7fffff]
    centers += [exponent << 23 | 0x352719 for exponent in range(1, 255, 13)]
    for center in centers:
        for offset in (-1, 0, 1):
            raw = center + offset
            if 0 <= raw <= 0x7f7fffff:
                for sign in (0, 0x80000000):
                    value = struct.unpack("!f", struct.pack("!I", raw | sign))[0]
                    tokens.append(format(value, ".9g"))
    tokens = list(dict.fromkeys(tokens))
    corpus = "\n".join(tokens) + "\n"
    (output / "inputs.txt").write_text(corpus)
    result = subprocess.run([str(binary)], input=corpus, text=True,
                            capture_output=True, check=True)
    header = "# token real angle angular_velocity velocity acceleration duration percent\n"
    (output / "ini_numeric_original.txt").write_text(header + result.stdout)
    (output / "command.txt").write_text(" ".join(command) + "\n")
    print(f"Generated {len(tokens)} rows / {len(tokens) * 7} original-source results")


if __name__ == "__main__":
    main()
