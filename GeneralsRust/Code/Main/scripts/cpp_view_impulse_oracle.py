#!/usr/bin/env python3
"""Compare the executed original W3DView impulse with committed C++ fixtures.

Reference profile: macOS ARM64 clang/math.h, not retail Windows execution.
No Rust code is executed and expected values are never regenerated. Outputs,
compiler logs and a machine-readable receipt stay outside the repository.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import platform
import re
import subprocess
from pathlib import Path

SOURCE = "GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DView.cpp"
BASE = "GeneralsMD/Code/Libraries/Include/Lib/BaseType.h"
FIXTURE = "GeneralsRust/Code/GameEngine/Common/src/common/view_impulse_cpp_cases.json"
PINS = {
    SOURCE: "0a62e7572862b8a3f75cb5d3427f19550f02dbf5d8920f55ced2ab98b11736ff",
    BASE: "590af4e59d0e48510d976eeafa248000a1ac9f05d8f905f66f24b1c1cf9eceef",
}


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def masked_cpp(text: str) -> str:
    """Preserve offsets while hiding comments and quoted braces."""
    pattern = r'//[^\n]*|/\*.*?\*/|"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\''
    return re.sub(pattern, lambda m: re.sub(r"[^\n]", " ", m[0]), text, flags=re.S)


def block_end(masked: str, opening: int) -> int:
    if masked[opening] != "{":
        raise ValueError("expected opening C++ block")
    depth = 0
    for index in range(opening, len(masked)):
        depth += (masked[index] == "{") - (masked[index] == "}")
        if depth == 0:
            return index + 1
    raise ValueError("unterminated C++ block")


def symbol(text: str, declaration: str) -> tuple[str, int]:
    masked = masked_cpp(text)
    matches = list(re.finditer(declaration, masked))
    if len(matches) != 1:
        raise ValueError(f"expected one source symbol, found {len(matches)}")
    start = matches[0].start()
    opening = masked.index("{", matches[0].end())
    return text[start:block_end(masked, opening)], text[:start].count("\n") + 1


def extract(text: str) -> tuple[str, str, dict]:
    shake, shake_line = symbol(text, r"void\s+W3DView::shake\s*\([^)]*\)")
    update, update_line = symbol(text, r"void\s+W3DView::update\s*\(void\)")
    masked = masked_cpp(update)
    matches = list(re.finditer(r"if\s*\(m_shakeIntensity\s*>\s*0\.01f\)", masked))
    if len(matches) != 1:
        raise ValueError("expected one update impulse threshold")
    start = matches[0].start()
    first_end = block_end(masked, masked.index("{", matches[0].end()))
    following = re.match(r"\s*else\s*\{", masked[first_end:])
    if following is None:
        raise ValueError("missing complete source quiet branch")
    end = block_end(masked, first_end + following.end() - 1)
    block = update[start:end]
    return shake, block, {
        "shake_start_line": shake_line,
        "shake_sha256": digest(shake.encode()),
        "update_start_line": update_line + update[:start].count("\n"),
        "update_block_sha256": digest(block.encode()),
    }


PREAMBLE = r'''
#include <math.h>
#include <cstdlib>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <initializer_list>
using Real = float;
#define PI 3.14159265359f
struct Coord3D { Real x,y,z; };
struct Rules {
 Real m_shakeSubtleIntensity,m_shakeNormalIntensity,m_shakeStrongIntensity;
 Real m_shakeSevereIntensity,m_shakeCineExtremeIntensity,m_shakeCineInsaneIntensity;
 Real m_maxShakeRange,m_maxShakeIntensity;
};
Rules rules; Rules* TheGlobalData=&rules;
float angleInput; unsigned drawCount;
uint32_t bits(float v) {uint32_t u;std::memcpy(&u,&v,4);return u;}
float number(uint32_t u) {float v;std::memcpy(&v,&u,4);return v;}
float GameClientRandomValueReal(float low,float high) {
 if(bits(low)!=0 || bits(high)!=bits(2*PI)) std::abort();
 ++drawCount;return angleInput;
}
struct W3DView {
 enum CameraShakeType {SHAKE_SUBTLE,SHAKE_NORMAL,SHAKE_STRONG,SHAKE_SEVERE,
                      SHAKE_CINE_EXTREME,SHAKE_CINE_INSANE};
 Real m_shakeIntensity,m_shakeAngleCos,m_shakeAngleSin;
 struct {Real x,y;} m_shakeOffset;
 Coord3D pos;
 const Coord3D* getPosition() {return &pos;}
 void shake(const Coord3D*,CameraShakeType);
 void updateShake() {bool recalcCamera=false;
'''
RUNNER = r'''
struct Event {uint32_t x,y,z,kind,angle;};
void emit(const char* name,const char* phase,int index,const W3DView& v) {
 std::printf("{\"case\":\"%s\",\"phase\":\"%s\",\"index\":%d,"
             "\"state_bits\":[%u,%u,%u,%u,%u],\"draws\":%u}\n",
             name,phase,index,bits(v.m_shakeIntensity),bits(v.m_shakeAngleCos),
             bits(v.m_shakeAngleSin),bits(v.m_shakeOffset.x),bits(v.m_shakeOffset.y),drawCount);
}
void run(const char* name,std::initializer_list<uint32_t> initial,
         std::initializer_list<uint32_t> position,uint32_t range,uint32_t maximum,
         std::initializer_list<uint32_t> intensities,
         std::initializer_list<Event> events,int steps) {
 auto s=initial.begin();W3DView v;
 v.m_shakeIntensity=number(*s++);v.m_shakeAngleCos=number(*s++);
 v.m_shakeAngleSin=number(*s++);v.m_shakeOffset.x=number(*s++);v.m_shakeOffset.y=number(*s++);
 auto p=position.begin();v.pos={number(*p++),number(*p++),number(*p++)};
 auto r=intensities.begin();
 rules={number(*r++),number(*r++),number(*r++),number(*r++),number(*r++),number(*r++),
        number(range),number(maximum)};
 drawCount=0;emit(name,"initial",0,v);int i=0;
 for(auto e:events) {
  angleInput=number(e.angle);Coord3D epicenter={number(e.x),number(e.y),number(e.z)};
  v.shake(&epicenter,static_cast<W3DView::CameraShakeType>(e.kind));emit(name,"enqueue",i++,v);
 }
 for(int k=0;k<steps;++k) {v.updateShake();emit(name,"step",k,v);}
}
int main() {
'''


def unsigned(value: int) -> str:
    if type(value) is not int or not 0 <= value <= 0xFFFFFFFF:
        raise ValueError("fixture value is not a u32 bit pattern")
    return str(value) + "u"


def vector(values: list[int], length: int) -> str:
    if len(values) != length:
        raise ValueError("unexpected fixture vector size")
    return "{" + ",".join(map(unsigned, values)) + "}"


def calls(cases: list) -> str:
    result = []
    names = set()
    for case in cases:
        item = case["input"]
        name = item["name"]
        if not re.fullmatch(r"[a-zA-Z0-9_]+", name) or name in names:
            raise ValueError("invalid or duplicate fixture name")
        names.add(name)
        steps = item["steps"]
        if type(steps) is not int or not 0 <= steps <= 1000:
            raise ValueError("invalid step count")
        events = []
        for event in item["events"]:
            if not 0 <= event["kind"] < 6:
                raise ValueError("invalid authored shake kind")
            events.append(vector(event["epicenter_bits"] + [event["kind"], event["angle_bits"]], 5))
        expected_count = 1 + len(events) + steps
        if len(case["expected"]) != expected_count:
            raise ValueError("fixture must cover every initial/enqueue/update row")
        result.append("run(" + ",".join([
            json.dumps(name), vector(item["initial_bits"], 5),
            vector(item["view_position_bits"], 3), unsigned(item["range_bits"]),
            unsigned(item["max_intensity_bits"]), vector(item["rule_intensities_bits"], 6),
            "{" + ",".join(events) + "}", str(steps),
        ]) + ");")
    return "\n".join(result)


def execute(args: argparse.Namespace, receipt: dict) -> None:
    root, output = args.repo_root.resolve(), args.output_root.resolve()
    if output.is_relative_to(root):
        raise ValueError("--output-root must be outside the repository")
    output.mkdir(parents=True, exist_ok=True)
    texts = {}
    for relative, expected_pin in PINS.items():
        data = (root / relative).read_bytes().replace(b"\r\n", b"\n")
        if digest(data) != expected_pin:
            raise ValueError(f"source pin mismatch: {relative}")
        texts[relative] = data.decode()
    fixture_bytes = (root / FIXTURE).read_bytes()
    cases = json.loads(fixture_bytes)
    if not isinstance(cases, list) or len(cases) != 22:
        raise ValueError("expected exactly 22 nonempty committed C++ cases")
    shake, update, extraction = extract(texts[SOURCE])
    receipt.update(extraction=extraction, source_pins=PINS, fixture_sha256=digest(fixture_bytes))
    (output / "shake-exact.cppfrag").write_text(shake)
    (output / "update-exact.cppfrag").write_text(update)
    harness = output / "harness.cpp"
    harness.write_text(PREAMBLE + update + "\n}\n};\n" + shake + RUNNER + calls(cases) + "\n}\n")
    command = [args.compiler, "-std=c++17", "-O0", "-ffp-contract=off", str(harness), "-o", str(output / "harness")]
    receipt["compiler_command"] = command
    version = subprocess.run([args.compiler, "--version"], capture_output=True, text=True, check=True)
    receipt["compiler_version"] = version.stdout.splitlines()[0]
    compiled = subprocess.run(command, capture_output=True, text=True)
    (output / "compile.log").write_text(compiled.stdout + compiled.stderr)
    receipt["compile_exit"] = compiled.returncode
    compiled.check_returncode()
    completed = subprocess.run([str(output / "harness")], capture_output=True, text=True, timeout=30)
    (output / "actual.jsonl").write_text(completed.stdout)
    (output / "run.stderr").write_text(completed.stderr)
    receipt["run_exit"] = completed.returncode
    completed.check_returncode()
    actual = [json.loads(line) for line in completed.stdout.splitlines() if line.strip()]
    expected = [row for case in cases for row in case["expected"]]
    if not actual or len(actual) != len(expected):
        raise ValueError(f"incomplete execution: {len(actual)} actual / {len(expected)} expected rows")
    differences = [{"row": i, "expected": e, "actual": a}
                   for i, (e, a) in enumerate(zip(expected, actual)) if e != a]
    (output / "differences.json").write_text(json.dumps(differences, indent=2) + "\n")
    receipt.update(cases=len(cases), rows=len(actual), differences=len(differences),
                   harness_sha256=digest(harness.read_bytes()),
                   actual_sha256=digest(completed.stdout.encode()))
    if differences:
        raise ValueError(f"original C++ disagrees with {len(differences)} committed fixture rows")
    receipt["status"] = "passed"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", required=True, type=Path)
    parser.add_argument("--output-root", required=True, type=Path)
    parser.add_argument("--compiler", default="clang++")
    args = parser.parse_args()
    receipt = {"status": "failed", "profile": "macOS ARM64 clang/math.h component; not retail Windows",
               "host": {"system": platform.system(), "machine": platform.machine()}}
    try:
        if platform.system() != "Darwin" or platform.machine() not in ("arm64", "aarch64"):
            raise ValueError("committed exact-bit fixture profile requires macOS ARM64")
        execute(args, receipt)
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        receipt["error"] = str(error)
    output = args.output_root.resolve()
    if not output.is_relative_to(args.repo_root.resolve()):
        output.mkdir(parents=True, exist_ok=True)
        (output / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt))
    return 0 if receipt["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
