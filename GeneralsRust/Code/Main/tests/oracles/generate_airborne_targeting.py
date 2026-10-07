#!/usr/bin/env python3
"""Execute pinned original integer/default/getter/AI flag statements only.

This is an extracted phase predicate, not an original game or loader execution.
The token supplier, template pointer, sampled height and flag storage are stubs.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

REPO = Path(__file__).resolve().parents[5]
PINS = {
    "GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Locomotor.cpp":
        "d0eb6b4cf7cac16a9d51e1456defc7b63ef97c70f0b356b7df3d77e005974f5a",
    "GeneralsMD/Code/GameEngine/Include/GameLogic/Locomotor.h":
        "298f952d2bc05aceeee10d25424267663164f0e6631f91145705bc91cdaa4c0c",
    "GeneralsMD/Code/GameEngine/Source/Common/INI/INI.cpp":
        "a8aa77e908d2482aa774408be94cc9b7a1fa13fa4b48d0ea64311e85a9e3a5e7",
    "GeneralsMD/Code/Libraries/Include/Lib/BaseType.h":
        "590af4e59d0e48510d976eeafa248000a1ac9f05d8f905f66f24b1c1cf9eceef",
    "GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate.cpp":
        "6ed6ea940b05acd368b4fd490733335007e44bfb4d281a89d228941aa50dba2c",
}
TOKENS = ["omitted", "0", "+0", "-0", "-1", "1", "-30", "30",
          "16777215", "16777216", "16777217", "-16777217", "2147483646",
          "2147483647", "-2147483648"]


def unique(pattern, text):
    matches = re.findall(pattern, text, re.M | re.S)
    assert len(matches) == 1, (pattern, len(matches))
    return matches[0]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--verify", type=Path)
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    sources = {}
    for path, pin in PINS.items():
        data = (REPO / path).read_bytes()
        assert hashlib.sha256(data).hexdigest() == pin, path
        sources[path] = data.decode()
    cpp, header, ini, base, ai = sources.values()
    unique(r'\{\s*"AirborneTargetingHeight",\s*INI::parseInt,\s*NULL,\s*offsetof\(\s*LocomotorTemplate,\s*m_airborneTargetingHeight\s*\)\s*\}', cpp)
    default = unique(r"\bm_airborneTargetingHeight = INT_MAX;", cpp)
    getter = unique(r"inline Int getAirborneTargetingHeight\(\) const \{[^}]+\}", header)
    parse = unique(r"void INI::parseInt\([^\n]+\n\{.*?\n\}", ini)
    scan = unique(r"Int INI::scanInt\(const char\* token\)\n\{.*?\n\}", ini)
    condition = unique(r"\t\tif\(getObject\(\)->getHeightAboveTerrain\(\) > m_curLocomotor->getAirborneTargetingHeight\(\) \)\n.*?clearStatus\([^\n]+;", ai)
    typedefs = [unique(r"^typedef[^;\n]+\b" + name + r";", base) for name in ("Real", "Int")]
    extracted = "\n".join([*typedefs, default, parse, scan, getter, condition]) + "\n"
    (out / "extracted_original_blocks.txt").write_text(extracted)
    source = r'''#include <cstdio>
#include <cstdint>
#include <cstring>
#include <climits>
#include <cmath>
#include <limits>
#include <iostream>
#include <string>
#include <vector>
''' + "\n".join(typedefs) + r'''
static_assert(sizeof(Int) == 4 && sizeof(Real) == 4 && INT_MAX == 2147483647);
enum { INI_INVALID_DATA = 1, OBJECT_STATUS_AIRBORNE_TARGET = 1 };
#define MAKE_OBJECT_STATUS_MASK(x) (x)
struct INI {
    const char* token;
    const char* getNextToken() { return token; }
    static Int scanInt(const char*);
    static void parseInt(INI*, void*, void*, const void*);
};
''' + scan + "\n" + parse + r'''
struct LocomotorTemplate {
    Int m_airborneTargetingHeight;
    LocomotorTemplate() { ''' + default + r''' }
};
struct Locomotor {
    LocomotorTemplate* m_template;
    ''' + getter + r'''
};
struct Object {
    Real height; bool flag;
    Real getHeightAboveTerrain() const { return height; }
    void setStatus(int) { flag = true; }
    void clearStatus(int) { flag = false; }
};
struct AIUpdate {
    Object* object; Locomotor* m_curLocomotor;
    Object* getObject() { return object; }
    void stamp() {
''' + condition + r'''
    }
};
std::uint32_t bits(Real value) { std::uint32_t r; std::memcpy(&r,&value,4); return r; }
int main() {
    std::string token;
    while (std::getline(std::cin, token)) {
        LocomotorTemplate t;
        if (token != "omitted") {
            INI ini{token.c_str()};
            INI::parseInt(&ini,nullptr,&t.m_airborneTargetingHeight,nullptr);
        }
        Locomotor loco{&t};
        const Real threshold = loco.getAirborneTargetingHeight();
        std::printf("load %s %d %08x\n", token.c_str(), t.m_airborneTargetingHeight, bits(threshold));
        std::vector<std::uint32_t> seen;
        for (Real height : {std::nextafter(threshold, -std::numeric_limits<Real>::infinity()),
                            threshold, std::nextafter(threshold, std::numeric_limits<Real>::infinity()),
                            Real(0), Real(1)}) {
            bool duplicate = false;
            for (auto previous : seen) if (previous == bits(height)) duplicate = true;
            if (duplicate) continue;
            seen.push_back(bits(height));
            for (bool initial : {false, true}) {
                Object object{height,initial}; AIUpdate update{&object,&loco}; update.stamp();
                std::printf("flag %s %d %08x %08x %d %d\n", token.c_str(),
                    t.m_airborneTargetingHeight, bits(threshold), bits(height), initial, object.flag);
            }
        }
    }
}
'''
    cpp_path = out / "airborne_targeting_original.cpp"
    cpp_path.write_text(source)
    binary = out / "airborne_targeting_original"
    command = ["c++", "-std=c++17", "-O0", "-ffp-contract=off", "-fno-fast-math",
               str(cpp_path), "-o", str(binary)]
    (out / "command.json").write_text(json.dumps(command, indent=2) + "\n")
    (out / "compiler_version.txt").write_text(subprocess.check_output(["c++", "--version"], text=True))
    subprocess.run(command, check=True)
    corpus = "\n".join(TOKENS) + "\n"
    (out / "inputs.txt").write_text(corpus)
    result = subprocess.run([str(binary)], input=corpus, text=True, capture_output=True, check=True)
    fixture = "# load token stored_i32 converted_float_bits\n# flag token stored_i32 threshold_bits height_bits initial final\n" + result.stdout
    (out / "airborne_targeting_original.txt").write_text(fixture)
    (out / "source_pins.json").write_text(json.dumps(PINS, indent=2) + "\n")
    if args.verify:
        assert args.verify.read_bytes() == fixture.encode(), "Frozen fixture differs"
    print(f"Generated {len(TOKENS)} loader rows and {sum(line.startswith('flag ') for line in fixture.splitlines())} predicate rows")


if __name__ == "__main__":
    main()
