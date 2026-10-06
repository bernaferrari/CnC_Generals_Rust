#!/usr/bin/env python3
"""Extract original approach statements into a native host arithmetic oracle.

This is not the original Windows executable or a game simulation. Phase inputs
exclude rotation, terrain checks, motive dispatch, force integration and timer
lifecycle. Original frame outputs and the explicit host adapter are separate.
"""

import argparse
import hashlib
from pathlib import Path
import re
import struct
import subprocess
import sys

REPO = Path(__file__).resolve().parents[5]
sys.path.insert(0, str(REPO / "GeneralsRust/Code/GameEngine/Common/tests/oracles"))
from generate_ini_numeric import PINS, extract_method
from generate_locomotor_numeric import LOCOMOTOR_PINS

EXTRA_PINS = {
    "GeneralsMD/Code/GameEngine/Include/GameLogic/GameLogic.h":
        "81e60073220b0ea6d51fc3b44b199a477b6f670b4e44c6b2032518a4e2448feb",
    "GeneralsMD/Code/GameEngine/Include/GameLogic/AIPathfind.h":
        "beb5a47ee8dc015a95bf8cdbbb800a64d3cfbb2cdbce4eaf940c70a2cf824d06",
}


def verify_pin(data, expected, name):
    assert hashlib.sha256(data).hexdigest() == expected, name


def span(source, start, end, occurrence=0):
    positions = [m.start() for m in re.finditer(re.escape(start), source)]
    assert len(positions) > occurrence, start
    begin = positions[occurrence]
    finish = source.index(end, begin)
    return source[begin:finish]


def u32(value):
    if isinstance(value, str):
        return int(value, 16)
    return struct.unpack("<I", struct.pack("<f", value))[0]


def trace_assignments(block):
    # Preserve every original statement; append a branch tag immediately after
    # its executed goal assignment. Never infer a copy from numeric equality.
    assignments = {
        "goalSpeed = actualSpeed-getBraking();": 1,
        "goalSpeed = actualSpeed-getBraking()/2.0f;": 4,
        "goalSpeed = actualSpeed;": 2,
        "goalSpeed = m_template->m_minSpeed;": 3,
    }
    for statement, tag in assignments.items():
        block = block.replace(statement, statement + f" choice = {tag};")
    return block


def cases():
    result = []

    def add(family, name, appearance="TREADS", **kw):
        row = dict(family=family, name=name, appearance=appearance, source="direct",
                   brake_token="unused", min_token="unused", actual=3., desired=4.,
                   minimum=.25, braking=.25, path=4., boundary="literal",
                   frame=100, no_slow=0, latched=0, factor=1.75, deadline=175)
        row.update(kw)
        result.append(row)

    for appearance in ("TREADS", "FOUR_WHEELS", "MOTORCYCLE"):
        family = "treads" if appearance == "TREADS" else "wheels"
        for label, path in (("full", 4.), ("half", 28. if family == "treads" else 34.),
                            ("hold", 40.)):
            add(family, f"{appearance}_{label}", appearance, path=path, latched=1)
        add(family, f"{appearance}_unlatched", appearance, path=100.)
        for boundary in ("slow_below", "slow_equal", "slow_above", "half_equal",
                         "half_above", "twice_equal", "twice_above"):
            add("boundaries", f"{appearance}_{boundary}", appearance,
                boundary=boundary, latched=1)
        for label, path, latched, deadline in (("new_latch_blocked", 4., 0, 175),
                                               ("inherited_processed", 4., 1, 175),
                                               ("local_clear", 100., 1, 175),
                                               ("expired_relatch", 39., 1, 99)):
            add("no_slow", f"{appearance}_{label}", appearance, path=path,
                no_slow=1, latched=latched, deadline=deadline, actual=1., braking=1.)
        for label, actual, braking in (("negative_speed", -3., .25),
                                      ("negative_brake", 3., -.25),
                                      ("positive_zero_brake", 3., 0.),
                                      ("negative_zero_brake", 3., -0.),
                                      ("zero_over_zero", 0., 0.),
                                      ("negative_zero_goal", -0., 0.),
                                      ("tiny_brake", 3., 1.e-9)):
            add("signed", f"{appearance}_{label}", appearance,
                actual=actual, braking=braking, path=0., latched=1)
        add("signed", f"{appearance}_negative_path", appearance, path=-3., latched=1)
        add("signed", f"{appearance}_zero_path_nan_factor", appearance,
            actual=0., path=0., latched=1)
    for path in ("cell_below", "cell_equal", "cell_above"):
        add("boundaries", "wheel_" + path, "FOUR_WHEELS", actual=1., braking=1.,
            boundary=path)
    for factor_path in (1., 10., 24., 40., 100.):
        add("treads", "factor_path_" + str(int(factor_path)), path=factor_path, latched=1)
    for boundary in ("slow_below", "slow_equal", "slow_above"):
        add("treads", "new_latch_" + boundary, boundary=boundary)
    for boundary in ("factor_below", "factor_equal", "factor_above"):
        add("boundaries", boundary, boundary=boundary, latched=1)
    for name, frame, deadline, path in (
            ("before_deadline", 100, 101, 40.), ("equal_deadline", 100, 100, 40.),
            ("after_deadline", 100, 99, 40.), ("far_refresh", 100, 99, 40.000004),
            ("round_2p24_minus", 16777215, 0, 41.),
            ("round_2p24_equal", 16777216, 0, 41.),
            ("round_2p24_plus", 16777217, 0, 41.),
            ("near_upper_defined_refresh", 4294966784, 0, 41.),
            ("wrap_before", 4294967294, 4294967294, 40.),
            ("wrap_expired", 4294967295, 4294967294, 40.),
            ("wrap_after", 0, 4294967294, 40.)):
        add("timer", name, "FOUR_WHEELS", actual=1., braking=1., no_slow=1,
            frame=frame, deadline=deadline, path=path)
    for appearance in ("TWO_LEGS", "CLIMBER", "OTHER", "HOVER", "WINGS"):
        add("generic", f"{appearance}_unlatched_stays_unlatched", appearance)
        for label, minimum, braking, no_slow in (
                ("minimum", .25, .25, 0), ("no_slow", .25, .25, 1),
                ("negative_minimum", -.25, .25, 0),
                ("negative_brake", .25, -.25, 0), ("zero_brake", .25, 0., 0),
                ("negative_zero_brake", .25, -0., 0),
                ("delta_equal", 3., 0., 0), ("delta_negative", 4., 0., 0)):
            # Negative Wings minimum is direct phase input, not template validation.
            add("generic", f"{appearance}_{label}", appearance, minimum=minimum,
                braking=braking, no_slow=no_slow, latched=1)
    for appearance in ("TREADS", "FOUR_WHEELS", "MOTORCYCLE", "TWO_LEGS",
                       "CLIMBER", "OTHER", "HOVER", "WINGS"):
        for token in ("100", "-100", "0", "-0", "0.0000009"):
            add("binding", f"{appearance}_{token}", appearance, source="load",
                brake_token=token, min_token="3", latched=1)
    add("binding", "negative_authored_minimum", "OTHER", source="load",
        brake_token="100", min_token="-3", latched=1)
    # Deliberately non-round-tripping authored values remain visible in both columns.
    add("binding", "roundtrip_boundary", "TREADS", source="load",
        brake_token="0.045", min_token="0.1", boundary="slow_equal", latched=1)
    add("binding", "default_braking_cap", "TREADS", source="load",
        brake_token="omitted", min_token="3", path=0., latched=1)
    # Large finite values expose the original double 2.0 local-clear expression.
    add("boundaries", "double_local_clear", actual=1.7e19, braking=1.,
        path="7f800000", latched=1)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--verify", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    sources = {}
    pins = {**PINS, **LOCOMOTOR_PINS, **EXTRA_PINS}
    for relative, expected in pins.items():
        data = (REPO / relative).read_bytes()
        verify_pin(data, expected, relative)
        sources[relative] = data.decode()
        if args.self_test:
            try:
                verify_pin(data + b" ", expected, relative)
            except AssertionError:
                pass
            else:
                raise AssertionError("changed-source negative control was accepted")
    ini, _, base = [sources[path] for path in PINS]
    cpp, header = [sources[path] for path in LOCOMOTOR_PINS]
    assert re.search(r"UnsignedInt\s+getFrame\(\s*void\s*\)", sources[next(iter(EXTRA_PINS))])
    assert re.search(r"UnsignedInt\s+m_donutTimer", header)
    assert '{ "Braking", INI::parseAccelerationReal' in cpp
    assert '{ "MinSpeed", INI::parseVelocityReal' in cpp
    bignum = re.search(r"const Real BIGNUM = [^;]+;", cpp)[0]
    template_ctor = cpp.split("LocomotorTemplate::LocomotorTemplate()", 1)[1].split("}", 1)[0]
    loco_ctor = cpp.split("Locomotor::Locomotor(const LocomotorTemplate* tmpl)", 1)[1].split("}", 1)[0]
    template_default = re.search(r"m_braking = [^;]+;", template_ctor)[0]
    cap_default = re.search(r"m_maxBraking = [^;]+;", loco_ctor)[0]
    getter = re.search(r"Real Locomotor::getBraking\(\) const\s*\{.*?\n\}", cpp, re.S)[0]
    path_header = sources["GeneralsMD/Code/GameEngine/Include/GameLogic/AIPathfind.h"]
    path_constants = "\n".join(re.search(r"^#define " + name + r"\s+[^\n]+", path_header, re.M)[0]
                                for name in ("PATHFIND_CELL_SIZE", "PATHFIND_CELL_SIZE_F"))
    slow = span(cpp, "static Real calcSlowDownDist(", "//-----------------------------------------------------------------------------")
    tread_calc = span(cpp, "\tReal slowDownTime = actualSpeed / getBraking();", "\n\tif (sqr(dx)")
    tread_phase = span(cpp, " \tif (onPathDistToGoal < slowDownDist &&", "\n\n\t//DEBUG_LOG")
    wheel_calc = span(cpp, "\tReal slowDownTime = actualSpeed / getBraking() + 1.0f;", "\n\n\tconst Real FIFTEEN_DEGREES")
    wheel_phase = span(cpp, "\tif (onPathDistToGoal < effectiveSlowDownDist &&", "\n\n\t//DEBUG_LOG")
    legs_phase = span(cpp, "\tReal slowDownDist = calcSlowDownDist(actualSpeed, m_template->m_minSpeed, getBraking());", "\n\n\n", 0)
    climb_phase = span(cpp, "\tReal slowDownDist = calcSlowDownDist(actualSpeed, m_template->m_minSpeed, getBraking());", "\n\t//", 1)
    other_phase = span(cpp, "\tif (!getFlag(NO_SLOW_DOWN_AS_APPROACHING_DEST))\n", "\n\t//", 0)
    wing_clear = span(cpp, "\tif ( \n// srj sez:", "\n\tBool wasBraking")
    blocks = dict(slow=slow, tread_calc=tread_calc, tread_phase=tread_phase,
                  wheel_calc=wheel_calc, wheel_phase=wheel_phase,
                  legs_phase=legs_phase, climb_phase=climb_phase,
                  other_phase=other_phase, wing_clear=wing_clear)
    (output / "extracted_original_blocks.txt").write_text("\n".join(
        "// " + name + "\n" + block for name, block in blocks.items()))
    declarations = [re.search(r"^typedef[^;]+\b" + name + r";", base, re.M)[0]
                    for name in ("Real", "Int", "UnsignedInt", "UnsignedShort", "Bool")]
    sqr = span(base, "template <typename NUM>\ninline NUM sqr", "\ntemplate <typename NUM>")
    pi = re.search(r"^#define PI\s+[^\n]+", base, re.M)[0]
    shim = output / "include/Lib"
    shim.mkdir(parents=True, exist_ok=True)
    (shim / "BaseType.h").write_text("#pragma once\n" + pi + "\n" + "\n".join(declarations) + "\n" + sqr)
    constants = "\n".join(re.search(pattern, cpp, re.M)[0] for pattern in (
        r"^static const Real DONUT_TIME_DELAY_SECONDS=[^;]+;",
        r"^static const Real DONUT_DISTANCE=[^;]+;", r"^#define MAX_BRAKING_FACTOR [^\n]+"))
    program = r'''#include <cstdio>
#include <cstdint>
#include <cstring>
#include <math.h>
#include <type_traits>
#include <cassert>
#include "Common/GameCommon.h"
static_assert(sizeof(Real) == 4 && sizeof(UnsignedInt) == 4);
// Explicit native math.h overload contract; original Windows compiler untested.
static_assert(std::is_same_v<decltype(fabs(Real{})), Real>);
enum { INI_INVALID_DATA = 1 };
''' + path_constants + r'''
struct INI {
    const char* token;
    const char* getNextToken() { return token; }
    static Real scanReal(const char*);
    static void parseVelocityReal(INI*, void*, void*, const void*);
    static void parseAccelerationReal(INI*, void*, void*, const void*);
};
''' + "\n".join(extract_method(ini, method) for method in
                 ("scanReal", "parseVelocityReal", "parseAccelerationReal")) + bignum + r'''
struct LocomotorTemplate { Real m_braking; LocomotorTemplate() { ''' + template_default + r''' } };
struct Locomotor {
    LocomotorTemplate* m_template; Real m_maxBraking;
    Locomotor(LocomotorTemplate* t) : m_template(t) { ''' + cap_default + r''' }
    Real getBraking() const;
};
''' + getter + constants + "\n" + slow + r'''
enum { IS_BRAKING, NO_SLOW_DOWN_AS_APPROACHING_DEST, LOCO_WINGS };
struct Clock { UnsignedInt frame; UnsignedInt getFrame() { return frame; } } clock_value;
Clock* TheGameLogic = &clock_value;
struct Template { Real m_minSpeed; int m_appearance; };
struct Result { Real goal, slow, factor; Bool flag; UnsignedInt deadline; int choice; };
struct Phase {
    Template* m_template;
    Real braking, m_brakingFactor;
    Bool flag, no_slow;
    UnsignedInt m_donutTimer;
    Bool getFlag(int which) { return which == IS_BRAKING ? flag : no_slow; }
    void setFlag(int, Bool value) { flag = value; }
    Real getBraking() { return braking; }
    Result treads(Real actualSpeed, Real goalSpeed, Real onPathDistToGoal) {
        int choice = 0;
''' + tread_calc + trace_assignments(tread_phase) + r'''
        return {goalSpeed, slowDownDist, m_brakingFactor, flag, m_donutTimer, choice};
    }
    Result wheels(Real actualSpeed, Real goalSpeed, Real onPathDistToGoal) {
        int choice = 0;
''' + wheel_calc + r'''
        // Exclude original undefined float-to-unsigned conversions BEFORE executing.
        if (onPathDistToGoal > DONUT_DISTANCE) {
            Real refreshed = TheGameLogic->getFrame()+DONUT_TIME_DELAY_SECONDS*LOGICFRAMES_PER_SECOND;
            assert(refreshed >= 0.0f && double(refreshed) < 4294967296.0);
        }
''' + trace_assignments(wheel_phase) + r'''
        return {goalSpeed, slowDownDist, m_brakingFactor, flag, m_donutTimer, choice};
    }
    Result legs(Real actualSpeed, Real goalSpeed, Real onPathDistToGoal) {
        int choice = 0;
''' + trace_assignments(legs_phase) + r'''
        return {goalSpeed, slowDownDist, m_brakingFactor, flag, m_donutTimer, choice};
    }
    Result climber(Real actualSpeed, Real goalSpeed, Real onPathDistToGoal) {
        int choice = 0;
''' + trace_assignments(climb_phase) + r'''
        return {goalSpeed, slowDownDist, m_brakingFactor, flag, m_donutTimer, choice};
    }
    Result other(Real actualSpeed, Real goalSpeed, Real onPathDistToGoal) {
        int choice = 0;
''' + trace_assignments(other_phase) + r'''
        Real distance = calcSlowDownDist(actualSpeed, m_template->m_minSpeed, braking);
        return {goalSpeed, distance, m_brakingFactor, flag, m_donutTimer, choice};
    }
    void clear_wing() {
''' + wing_clear + r'''
    }
};
std::uint32_t bits(Real v) { std::uint32_t b; std::memcpy(&b, &v, 4); return b; }
Real f(std::uint32_t b) { Real v; std::memcpy(&v, &b, 4); return v; }
struct Case {
    const char *family, *name, *appearance, *source, *brake_token, *min_token, *boundary;
    std::uint32_t actual, desired, minimum, braking, path, factor, frame, deadline;
    Bool no_slow, flag;
};
Result run(const Case& c, Real v, Real d, Real m, Real b, Real path) {
    Template t{m, strcmp(c.appearance, "WINGS") == 0 ? LOCO_WINGS : -1};
    Phase p{&t, b, f(c.factor), c.flag, c.no_slow, c.deadline};
    clock_value.frame = c.frame;
    if (!strcmp(c.appearance, "TREADS")) return p.treads(v, d, path);
    if (!strcmp(c.appearance, "FOUR_WHEELS") || !strcmp(c.appearance, "MOTORCYCLE")) return p.wheels(v, d, path);
    if (!strcmp(c.appearance, "TWO_LEGS")) return p.legs(v, d, path);
    if (!strcmp(c.appearance, "CLIMBER")) return p.climber(v, d, path);
    if (!strcmp(c.appearance, "WINGS")) p.clear_wing();
    return p.other(v, d, path);
}
int main() {
    const Case cases[] = {
'''
    for row in cases():
        strings = ", ".join('"' + row[key] + '"' for key in
                            ("family", "name", "appearance", "source", "brake_token", "min_token", "boundary"))
        floats = ", ".join(f"0x{u32(row[key]):08x}u" for key in
                           ("actual", "desired", "minimum", "braking", "path", "factor"))
        program += f"        {{{strings}, {floats}, {row['frame']}u, {row['deadline']}u, {row['no_slow']}, {row['latched']}}},\n"
    program += r'''
    };
    for (const Case& c : cases) {
        Real v = f(c.actual), d = f(c.desired), m = f(c.minimum), b = f(c.braking);
        Real vh = v*30.0f, dh = d*30.0f, mh = m*30.0f, bh = b*30.0f*30.0f;
        Real parsed = b;
        if (!strcmp(c.source, "load")) {
            INI bi{c.brake_token}, mi{c.min_token};
            LocomotorTemplate authored;
            if (strcmp(c.brake_token, "omitted")) INI::parseAccelerationReal(&bi, nullptr, &authored.m_braking, nullptr);
            parsed = authored.m_braking;
            Locomotor loco(&authored);
            b = loco.getBraking();
            INI::parseVelocityReal(&mi, nullptr, &m, nullptr);
            mh = INI::scanReal(c.min_token);
            bh = b*30.0f*30.0f;
        }
        Real vr = vh/30.0f, dr = dh/30.0f, mr = mh/30.0f, br = bh/30.0f/30.0f;
        Real path = f(c.path);
        // Boundary selection prepares explicit scalar input, before either phase.
        // For wheels this probe uses a non-refreshing path and an initialized timer.
        Case probe = c; probe.frame = 0; probe.deadline = 1;
        Real distance = run(probe, vr, dr, mr, br, 0.0f).slow;
        if (!strcmp(c.boundary, "slow_below")) path = nextafterf(distance, -INFINITY);
        if (!strcmp(c.boundary, "slow_equal")) path = distance;
        if (!strcmp(c.boundary, "slow_above")) path = nextafterf(distance, INFINITY);
        if (!strcmp(c.boundary, "half_equal")) path = distance/0.75f;
        if (!strcmp(c.boundary, "half_above")) path = nextafterf(distance/0.75f, INFINITY);
        if (!strcmp(c.boundary, "twice_equal")) path = 2.0f*distance;
        if (!strcmp(c.boundary, "twice_above")) path = nextafterf(2.0f*distance, INFINITY);
        if (!strcmp(c.boundary, "cell_below")) path = nextafterf(10.0f, 0.0f);
        if (!strcmp(c.boundary, "cell_equal")) path = 10.0f;
        if (!strcmp(c.boundary, "cell_above")) path = nextafterf(10.0f, INFINITY);
        if (!strcmp(c.boundary, "factor_below")) path = nextafterf(distance/sqrtf(5.0f), INFINITY);
        if (!strcmp(c.boundary, "factor_equal")) path = distance/sqrtf(5.0f);
        if (!strcmp(c.boundary, "factor_above")) path = nextafterf(distance/sqrtf(5.0f), 0.0f);
        Result original = run(c, v, d, m, b, path);
        Result reconstructed = run(c, vr, dr, mr, br, path);
        Real host = reconstructed.choice == 0 ? dh : (reconstructed.choice == 1 || reconstructed.choice == 4) ?
            reconstructed.goal*30.0f : reconstructed.choice == 2 ? vh : mh;
        std::printf("%s %s %s %s %s %s %u %d %d %08x %u %08x %08x %08x %08x %08x "
            "%08x %08x %08x %08x %08x %08x %08x %08x %08x "
            "%08x %08x %d %08x %u %d %08x %08x %d %08x %u %d %08x\n",
            c.family,c.name,c.appearance,c.source,c.brake_token,c.min_token,c.frame,c.no_slow,c.flag,c.factor,c.deadline,
            bits(path),bits(vh),bits(dh),bits(mh),bits(bh),
            bits(parsed),bits(v),bits(d),bits(m),bits(b),bits(vr),bits(dr),bits(mr),bits(br),
            bits(original.slow),bits(original.goal),original.flag,bits(original.factor),original.deadline,original.choice,
            bits(reconstructed.slow),bits(reconstructed.goal),reconstructed.flag,bits(reconstructed.factor),
            reconstructed.deadline,reconstructed.choice,bits(host));
    }
    // Direct shared-helper controls use its own scalar units, without host adaptation.
    const Real controls[][3] = {{3,1,.25f},{3,1,-.25f},{3,1,0},{3,1,-0.0f},
        {3,3,0},{2,3,0},{0,-1,1e-9f},{-3,-4,-.25f},{1e18f,0,.01f}};
    unsigned index = 0;
    for (auto& c : controls) std::printf("helper helper_%u %08x %08x %08x %08x\n",
        index++, bits(c[0]),bits(c[1]),bits(c[2]),bits(calcSlowDownDist(c[0],c[1],c[2])));
}
'''
    source = output / "approach_original.cpp"
    source.write_text(program)
    binary = output / "approach_original"
    command = ["c++", "-std=c++17", "-O0", "-ffp-contract=off", "-fno-fast-math",
               "-I" + str(output / "include"),
               "-I" + str(REPO / "GeneralsMD/Code/GameEngine/Include"), str(source), "-o", str(binary)]
    subprocess.run(command, check=True)
    result = subprocess.run([str(binary)], text=True, capture_output=True, check=True)
    columns = ("family name appearance source brake_token min_token frame no_slow initial_flag initial_factor initial_deadline "
               "path host_actual host_desired host_min host_brake parsed_brake raw_actual raw_desired raw_min raw_brake "
               "reconstructed_actual reconstructed_desired reconstructed_min reconstructed_brake "
               "raw_slow raw_goal raw_flag raw_factor raw_deadline raw_choice "
               "reconstructed_slow reconstructed_goal final_flag final_factor final_deadline choice host_goal")
    fixture = "# " + columns + "\n# helper name current desired braking result\n" + result.stdout
    (output / "approach_original.txt").write_text(fixture)
    (output / "command.txt").write_text(" ".join(command) + "\n")
    (output / "source_pins.txt").write_text("".join(f"{v}  {k}\n" for k, v in pins.items()))
    if args.verify:
        assert args.verify.read_text() == fixture, "fixture differs from extracted original output"
    print(f"Generated {len(cases())} approach rows and 9 helper rows; native math.h fabs(float) -> float")


if __name__ == "__main__":
    main()
