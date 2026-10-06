#!/usr/bin/env python3
"""Pinned, extracted C++ braking arithmetic, not a whole-engine simulation.

The original scan/callback/converter and getter run unchanged. Constructors
contribute only their exact braking assignments; the token source and unrelated
state are stubs. The host sec^2 column is an explicitly labeled adapter result.
"""

import argparse
import hashlib
from pathlib import Path
import re
import subprocess
import sys


REPO = Path(__file__).resolve().parents[5]
sys.path.insert(0, str(REPO / "GeneralsRust/Code/GameEngine/Common/tests/oracles"))
from generate_ini_numeric import PINS, extract_method
from generate_locomotor_numeric import LOCOMOTOR_PINS


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    sources = {}
    for relative, expected in {**PINS, **LOCOMOTOR_PINS}.items():
        data = (REPO / relative).read_bytes()
        assert hashlib.sha256(data).hexdigest() == expected, relative
        sources[relative] = data.decode()
    ini, _, base = [sources[path] for path in PINS]
    cpp, header = [sources[path] for path in LOCOMOTOR_PINS]
    assert '{ "Braking", INI::parseAccelerationReal, NULL, offsetof(LocomotorTemplate, m_braking) }' in cpp
    assert re.search(r"\bReal\s+m_braking\s*;", header)
    assert re.search(r"\bReal\s+m_maxBraking\s*;", header)
    bignum = re.search(r"const Real BIGNUM = [^;]+;", cpp)[0]
    template_ctor = cpp.split("LocomotorTemplate::LocomotorTemplate()", 1)[1].split("}", 1)[0]
    loco_ctor = cpp.split("Locomotor::Locomotor(const LocomotorTemplate* tmpl)", 1)[1].split("}", 1)[0]
    template_default = re.search(r"m_braking = [^;]+;", template_ctor)[0]
    cap_default = re.search(r"m_maxBraking = [^;]+;", loco_ctor)[0]
    getter = re.search(r"Real Locomotor::getBraking\(\) const\s*\{.*?\n\}", cpp, re.S)[0]
    shim = output / "include/Lib"
    shim.mkdir(parents=True, exist_ok=True)
    declarations = [re.search(r"^typedef[^;]+\b" + name + r";", base, re.M)[0]
                    for name in ("Real", "Int", "UnsignedInt", "UnsignedShort", "Bool")]
    pi = re.search(r"^#define PI\s+[^\n]+", base, re.M)[0]
    (shim / "BaseType.h").write_text("#pragma once\n" + pi + "\n" + "\n".join(declarations) + "\n")
    program = '''#include <cstdio>
#include <cstdint>
#include <cstring>
#include <string>
#include <iostream>
#include "Common/GameCommon.h"
static_assert(sizeof(Real) == 4);
enum { INI_INVALID_DATA = 1 };
struct INI {
    const char* token;
    const char* getNextToken() { return token; }
    static Real scanReal(const char*);
    static void parseAccelerationReal(INI*, void*, void*, const void*);
};
''' + extract_method(ini, "scanReal") + "\n" + extract_method(ini, "parseAccelerationReal") + "\n" + bignum + '''
struct LocomotorTemplate {
    Real m_braking;
    LocomotorTemplate() { ''' + template_default + ''' }
};
struct Locomotor {
    LocomotorTemplate* m_template;
    Real m_maxBraking;
    Locomotor(LocomotorTemplate* value) : m_template(value) { ''' + cap_default + ''' }
    Real getBraking() const;
};
''' + getter + '''
std::uint32_t bits(Real value) {
    std::uint32_t result; std::memcpy(&result, &value, 4); return result;
}
Real from_bits(std::uint32_t value) {
    Real result; std::memcpy(&result, &value, 4); return result;
}
int main() {
    std::string token;
    while (std::getline(std::cin, token)) {
        LocomotorTemplate t;
        if (token != "omitted") {
            INI ini{token.c_str()};
            INI::parseAccelerationReal(&ini, nullptr, &t.m_braking, nullptr);
        }
        Locomotor loco(&t);
        // Adapter arithmetic only: original effective frame^2 -> host sec^2.
        Real host = loco.getBraking() * 30.0f * 30.0f;
        std::printf("load %s %08x %08x %08x %08x\\n", token.c_str(),
            bits(t.m_braking), bits(loco.m_maxBraking), bits(loco.getBraking()), bits(host));
    }
    const std::uint32_t pairs[][2] = {
        {0x00000000, 0x80000000}, {0x80000000, 0x00000000},
        {0x7fc01234, 0x47c34f80}, {0x3f800000, 0x7fc01234},
        {0x7f800000, 0x47c34f80}, {0xff800000, 0x47c34f80},
        {0x41200000, 0x3f800000}, {0xc1200000, 0xbf800000}
    };
    for (auto pair : pairs) {
        LocomotorTemplate t;
        t.m_braking = from_bits(pair[0]);
        Locomotor loco(&t);
        loco.m_maxBraking = from_bits(pair[1]);
        std::printf("cap %08x %08x %08x\\n", pair[0], pair[1], bits(loco.getBraking()));
    }
}
'''
    source = output / "braking_original.cpp"
    source.write_text(program)
    binary = output / "braking_original"
    command = ["c++", "-std=c++17", "-O0", "-ffp-contract=off", "-fno-fast-math",
               "-I" + str(output / "include"),
               "-I" + str(REPO / "GeneralsMD/Code/GameEngine/Include"), str(source), "-o", str(binary)]
    subprocess.run(command, check=True)
    tokens = ["omitted", "0", "-0", "100", "-100", "1.40129846e-45", "-1.40129846e-45",
              "99999", "89999000", "89999100", "89999200", "90000000", "3.40282347e+38", "-3.40282347e+38"]
    result = subprocess.run([str(binary)], input="\n".join(tokens) + "\n", text=True,
                            capture_output=True, check=True)
    (output / "braking_original.txt").write_text(
        "# load token template_frame2 cap_frame2 effective_frame2 adapter_sec2\n"
        "# cap template_frame2 cap_frame2 effective_frame2 (hex float bits)\n" + result.stdout)
    (output / "command.txt").write_text(" ".join(command) + "\n")
    (output / "source_pins.txt").write_text("".join(
        f"{value}  {key}\n" for key, value in {**PINS, **LOCOMOTOR_PINS}.items()))
    print(f"Generated {len(tokens)} loader/default rows and 8 ordered-comparison controls")


if __name__ == "__main__":
    main()
