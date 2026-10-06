#!/usr/bin/env python3
"""Pinned original OTHER force block, with raw-frame and host-impulse runs.

This is an extracted scalar oracle, not the original game executable. The force
block is unchanged. The host run supplies sec-speed gaps and sec-speed impulses;
its output is an explicitly adapted expectation for Main's mass-one path.
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

PHYSICS = "GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/PhysicsUpdate.cpp"
PINS_ALL = {**PINS, **LOCOMOTOR_PINS, PHYSICS:
            "95b47641e6517b18ec62e50a7b0bbffb03181dee89d5cc1b5fa2495b83db31d6"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--verify", type=Path)
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    sources = {}
    for path, pin in PINS_ALL.items():
        data = (REPO / path).read_bytes()
        assert hashlib.sha256(data).hexdigest() == pin, path
        sources[path] = data.decode()
    ini, _, base = [sources[path] for path in PINS]
    cpp, _ = [sources[path] for path in LOCOMOTOR_PINS]
    other = cpp.split("void Locomotor::moveTowardsPositionOther(", 1)[1]
    block = other[other.index("\tReal speedDelta ="):other.index("\n\n}\n")]
    assert "if (fabs(accelForce) > fabs(maxForceNeeded))" in block
    force_math = sources[PHYSICS].split("void PhysicsBehavior::applyForce(", 1)[1]
    begin = force_math.index("\tReal massInv =")
    end = force_math.index("\n\n\t//DEBUG_ASSERTCRASH", begin)
    force_math = force_math[begin:end]
    velocity_line = "\t\tm_vel.x += m_accel.x;"
    position_line = "\t\t\tmtx.Adjust_X_Translation(m_vel.x);"
    assert velocity_line in sources[PHYSICS] and position_line in sources[PHYSICS]
    extracted = block + "\n" + force_math + "\n" + velocity_line + "\n" + position_line + "\n"
    (out / "extracted_original_blocks.txt").write_text(extracted)
    shim = out / "include/Lib"
    shim.mkdir(parents=True, exist_ok=True)
    declarations = [re.search(r"^typedef[^;]+\b" + name + r";", base, re.M)[0]
                    for name in ("Real", "Int", "UnsignedInt", "UnsignedShort", "Bool")]
    pi = re.search(r"^#define PI\s+[^\n]+", base, re.M)[0]
    (shim / "BaseType.h").write_text("#pragma once\n" + pi + "\n" + "\n".join(declarations) + "\n")
    program = r'''#include <cstdio>
#include <cstdint>
#include <cstring>
#include <math.h>
#include "Common/GameCommon.h"
static_assert(sizeof(Real) == 4);
static_assert(sizeof(fabs(Real{})) == sizeof(Real));
enum { INI_INVALID_DATA = 1 };
struct INI {
    const char* token;
    const char* getNextToken() { return token; }
    static Real scanReal(const char*);
    static void parseAccelerationReal(INI*, void*, void*, const void*);
    static void parseVelocityReal(INI*, void*, void*, const void*);
};
''' + extract_method(ini, "scanReal") + "\n" + extract_method(ini, "parseAccelerationReal") + "\n" + extract_method(ini, "parseVelocityReal") + r'''
struct Coord3D { Real x=0, y=0, z=0; };
struct Matrix { Real x=0; void Adjust_X_Translation(Real v) { x += v; } };
struct Physics {
    Real requested=0; bool called=false;
    Coord3D m_accel, m_vel;
    Real getMass() const { return 1.0f; }
    void applyMotiveForce(const Coord3D* force) {
        requested = force->x; called = true;
        // applyMotiveForce clears motive before applyForce; no lateral-only projection.
        Real mass = getMass(); Coord3D modForce = *force;
''' + force_math + r'''
    }
};
struct Result { Real force, next, position; bool called; };
struct Probe {
    Real braking;
    Real getBraking() const { return braking; }
    Result run(Real actualSpeed, Real goalSpeed, Real maxAcceleration) {
        Physics body; Physics* physics = &body;
        Coord3D dirToApplyForce{1.0f,0.0f,0.0f};
''' + block + r'''
        Coord3D m_vel{actualSpeed,0.0f,0.0f}; Coord3D m_accel = body.m_accel;
''' + velocity_line + r'''
        Matrix mtx;
''' + position_line + r'''
        return {body.requested, m_vel.x, mtx.x, body.called};
    }
};
std::uint32_t bits(Real value) { std::uint32_t r; std::memcpy(&r,&value,4); return r; }
struct Case { const char* name; const char* token; bool zero_gap; };
int main() {
    const Case cases[] = {
        {"negative_large", "-1800", false},
        {"negative_below", "-450", false},
        {"negative_equal", "-899.99981689453125", false},
        {"positive_large", "1800", false},
        {"positive_below", "450", false},
        {"positive_equal", "899.99981689453125", false},
        {"positive_zero", "0", false},
        {"negative_zero", "-0", false},
        {"zero_gap", "-1800", true}
    };
    for (auto c : cases) {
        Real raw_brake, raw_goal, raw_accel, raw_actual;
        INI brake{c.token}, goal{"60"}, accel{"30"}, actual{c.zero_gap ? "60" : "90"};
        INI::parseAccelerationReal(&brake,nullptr,&raw_brake,nullptr);
        INI::parseAccelerationReal(&accel,nullptr,&raw_accel,nullptr);
        INI::parseVelocityReal(&goal,nullptr,&raw_goal,nullptr);
        INI::parseVelocityReal(&actual,nullptr,&raw_actual,nullptr);
        const Real dt=1.0f/30.0f;
        Real host_goal=raw_goal*30.0f;
        Real host_brake=raw_brake*30.0f*30.0f;
        Real host_actual=c.zero_gap ? host_goal : 90.0f;
        Real host_accel=raw_accel*30.0f*30.0f;
        Result raw=Probe{raw_brake}.run(raw_actual,raw_goal,raw_accel);
        Result reconstructed=Probe{host_brake/30.0f/30.0f}.run(host_actual/30.0f,host_goal/30.0f,host_accel/30.0f/30.0f);
        // Adapter input is a seconds-speed impulse. Do not label this raw frame arithmetic.
        Result adapted=Probe{host_brake*dt}.run(host_actual,host_goal,host_accel*dt);
        std::printf("live %s %s %08x %08x %08x %08x %08x %08x %08x %08x %08x %08x %08x %08x %08x %08x %08x %08x %d\n",
            c.name,c.token,bits(host_actual),bits(host_goal),bits(host_brake),
            bits(raw_actual),bits(raw_goal),bits(raw_brake),bits(raw.force),bits(raw.next),bits(raw.position),
            bits(reconstructed.force),bits(reconstructed.next),bits(adapted.force),bits(adapted.next),
            bits(adapted.next*dt),bits(host_brake*dt),bits(host_goal-host_actual),adapted.called);
    }
    const Real brakes[]={-2,-.5f,-1,2,.5f,1,0.0f,-0.0f};
    unsigned index=0;
    for (Real b : brakes) {
        Result r=Probe{b}.run(3,2,1);
        std::printf("scalar %u %08x %08x %08x %d\n",index++,bits(b),bits(r.force),bits(r.next),r.called);
    }
    Result r=Probe{-2}.run(2,2,1);
    std::printf("scalar 8 %08x %08x %08x %d\n",bits(-2.0f),bits(r.force),bits(r.next),r.called);
}
'''
    source = out / "signed_braking_original.cpp"
    source.write_text(program)
    binary = out / "signed_braking_original"
    command = ["c++", "-std=c++17", "-O0", "-Werror=format", "-ffp-contract=off", "-fno-fast-math",
               "-I" + str(out / "include"), "-I" + str(REPO / "GeneralsMD/Code/GameEngine/Include"),
               str(source), "-o", str(binary)]
    subprocess.run(command, check=True)
    result = subprocess.run([str(binary)], text=True, capture_output=True, check=True)
    fixture = ("# live name token host_actual host_goal host_brake raw_actual raw_goal raw_brake "
               "raw_force raw_next raw_position reconstructed_force reconstructed_next "
               "adapted_impulse adapted_next adapted_x host_step host_gap called\n"
               "# scalar index brake force next called (all scalars hexadecimal f32 bits)\n" + result.stdout)
    (out / "signed_braking_original.txt").write_text(fixture)
    (out / "command.txt").write_text(" ".join(command) + "\n")
    (out / "source_pins.txt").write_text("".join(f"{v}  {k}\n" for k,v in PINS_ALL.items()))
    if args.verify:
        assert args.verify.read_text() == fixture, "fixture differs from extracted original output"
    print("Generated 9 live rows and 9 exact frame-scalar controls")

if __name__ == "__main__":
    main()
