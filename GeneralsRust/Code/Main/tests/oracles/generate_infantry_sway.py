#!/usr/bin/env python3
"""Execute pinned original infantry wander arithmetic, with a labelled host adapter.

Reads local original files and rejects any byte drift from the pinned source.
This is a scalar extraction,
not the original game, its constructor/RNG, orientation matrices or dispatcher.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

REVISION = "5ad1b5dac509c7c2574250b9f9d00542ede9b652"
DEFAULT_REPO = Path(__file__).resolve().parents[5]
PINS = {
    "GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Locomotor.cpp":
        "d0eb6b4cf7cac16a9d51e1456defc7b63ef97c70f0b356b7df3d77e005974f5a",
    "GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/PhysicsUpdate.cpp":
        "95b47641e6517b18ec62e50a7b0bbffb03181dee89d5cc1b5fa2495b83db31d6",
    "GeneralsMD/Code/GameEngine/Source/Common/System/GameCommon.cpp":
        "d18d9f8ff9cee817d7bf88b370dc2e4a701f98979c85cd3bd0c712bd18818157",
    "GeneralsMD/Code/GameEngine/Include/Common/GameCommon.h":
        "5765730dca375098a638184ce473cfbe43fed12623a1e8234ccd219883a5e54a",
    "GeneralsMD/Code/Libraries/Include/Lib/BaseType.h":
        "590af4e59d0e48510d976eeafa248000a1ac9f05d8f905f66f24b1c1cf9eceef",
}
# The file pins reject changed source; independent block pins reject shifted ranges.
BLOCKS = {
    "forward_speed": (list(PINS)[1], 939, 957,
        "e96544dc5db5218e021cbc75fa5e28af0ce4b49ac9a3971fdf22ba9c73f177b1"),
    "normalize_angle": (list(PINS)[2], 27, 41,
        "47c003df30a3a56dcc3081a3b892c18aae2afae308d45544b40c5f3d9bc4c170"),
    "wander_width_block": (list(PINS)[0], 1618, 1633,
        "183a656946b7a29ce25bc4bf01b94f547c12e124d1930f8e90bf3824e792f668"),
    "frame_conversions": (list(PINS)[3], 37, 76,
        "8928e39b4e5a0d32a2d131c4b75785744e34f3f32bf6caf6c2fde6a473db12c8"),
}

HEADER = """# Pinned original-extracted infantry wander arithmetic, not original game execution.
# Every scalar is 8 hexadecimal f32 bits; flags and sequence indices are decimal.
# live name host_vx host_vy host_vz heading goal_heading offset increment width increasing raw_frame_forward raw_offset raw_increasing raw_desired host_forward adapted_frame_forward adapted_offset adapted_increasing adapted_desired
# Raw lane: ConvertVelocityInSecsToFrames on components, then original speed and wander.
# Adapted lane: original speed on host components, then ConvertVelocityInSecsToFrames, then original wander.
# Live directions are supplied by harness cosf/sinf; no original direction-vector or matrix body is executed.
# scalar name actual_frame_speed desired offset increment width increasing next_offset next_increment next_increasing next_desired
# sequence name index actual_frame_speed desired offset increment width increasing next_offset next_increment next_increasing next_desired
# speed name original_vx original_vy original_vz supplied_dir_x supplied_dir_y signed_component_product_speed
# normalize name input output
"""

PREFIX = r'''#include <cstdio>
#include <cstdint>
#include <cstring>
#include <cmath>
#include <limits>
#include <math.h>
// Release-style assertion stub; original explicit NaN handling remains executed.
#define DEBUG_ASSERTCRASH(condition, message) ((void)0)
#define _isnan(value) std::isnan(value)
'''

SHIMS = r'''
static_assert(sizeof(Real) == 4);
static_assert(std::numeric_limits<Real>::is_iec559);
struct Coord3D { Real x, y, z; };
struct Object {
    Coord3D direction;
    const Coord3D* getUnitDirectionVector2D() const { return &direction; }
};
struct PhysicsBehavior {
    Coord3D m_vel;
    Object object;
    const Object* getObject() const { return &object; }
    Real getForwardSpeed2D() const;
};
'''

LOCOMOTOR_PREFIX = r'''
struct LocomotorTemplate { Real m_wanderWidthFactor; };
struct Result { Real offset, increment; bool increasing; Real desired; };
struct Locomotor {
    LocomotorTemplate owned_template;
    const LocomotorTemplate* m_template = &owned_template;
    Real m_angleOffset;
    Real m_offsetIncrement;
    bool increasing;
    enum { OFFSET_INCREASING = 11 };
    Locomotor(Real width, Real offset, Real increment, bool direction)
        : owned_template{width}, m_angleOffset(offset),
          m_offsetIncrement(increment), increasing(direction) {}
    bool getFlag(int) const { return increasing; }
    void setFlag(int, bool value) { increasing = value; }
    Result step(Real actualSpeed, Real desiredAngle) {
'''

LOCOMOTOR_SUFFIX = r'''
        return {m_angleOffset, m_offsetIncrement, increasing, desiredAngle};
    }
};
std::uint32_t bits(Real value) {
    std::uint32_t result; std::memcpy(&result, &value, sizeof(result)); return result;
}
Real from_bits(std::uint32_t value) {
    Real result; std::memcpy(&result, &value, sizeof(result)); return result;
}
void hex(Real value) { std::printf(" %08x", bits(value)); }
void flag(bool value) { std::printf(" %d", value ? 1 : 0); }
void result(Result value) {
    hex(value.offset); hex(value.increment); flag(value.increasing); hex(value.desired);
}
Real original_speed(Coord3D velocity, Real dx, Real dy) {
    PhysicsBehavior body{velocity, Object{Coord3D{dx, dy, 0.0f}}};
    return body.getForwardSpeed2D();
}
struct Case {
    const char* name;
    Real host_vx, host_vy, host_vz;
    Real heading=0.0f, goal=0.0f;
    Real offset=0.1875f, increment=0.0625f, width=1.0f;
    bool increasing=true;
};
void live(Case c) {
    // ADAPTATION: these trig-supplied components are not an extracted original
    // Thing/Object matrix calculation. C++ XY = host (X,-Z), C++ Z = host Y.
    const Real dx = cosf(c.heading), dy = sinf(c.heading);
    const Coord3D host{c.host_vx, -c.host_vz, c.host_vy};
    const Coord3D raw{
        ConvertVelocityInSecsToFrames(host.x),
        ConvertVelocityInSecsToFrames(host.y),
        ConvertVelocityInSecsToFrames(host.z)};
    const Real raw_speed = original_speed(raw, dx, dy);
    const Real host_speed = original_speed(host, dx, dy);
    const Real adapted_speed = ConvertVelocityInSecsToFrames(host_speed);
    Locomotor raw_loco{c.width,c.offset,c.increment,c.increasing};
    Locomotor adapted_loco{c.width,c.offset,c.increment,c.increasing};
    const Result raw_result = raw_loco.step(raw_speed,c.goal);
    const Result adapted_result = adapted_loco.step(adapted_speed,c.goal);
    std::printf("live %s", c.name);
    hex(c.host_vx); hex(c.host_vy); hex(c.host_vz); hex(c.heading); hex(c.goal);
    hex(c.offset); hex(c.increment); hex(c.width); flag(c.increasing);
    hex(raw_speed); hex(raw_result.offset); flag(raw_result.increasing); hex(raw_result.desired);
    hex(host_speed); hex(adapted_speed); hex(adapted_result.offset);
    flag(adapted_result.increasing); hex(adapted_result.desired); std::puts("");
}
void scalar(const char* name, Real speed, Real desired, Real offset,
            Real increment, Real width, bool increasing) {
    Locomotor loco{width,offset,increment,increasing};
    std::printf("scalar %s", name);
    hex(speed); hex(desired); hex(offset); hex(increment); hex(width); flag(increasing);
    result(loco.step(speed,desired)); std::puts("");
}
void speed(const char* name, Coord3D velocity, Real dx, Real dy) {
    std::printf("speed %s",name);
    hex(velocity.x); hex(velocity.y); hex(velocity.z); hex(dx); hex(dy);
    hex(original_speed(velocity,dx,dy)); std::puts("");
}
void normalize(const char* name, Real input) {
    std::printf("normalize %s",name); hex(input); hex(normalizeAngle(input)); std::puts("");
}
void sequence(const char* name, const Real* speeds, unsigned count,
              Real offset, Real increment, Real width, bool increasing) {
    Locomotor loco{width,offset,increment,increasing};
    for (unsigned index=0; index<count; ++index) {
        std::printf("sequence %s %u",name,index);
        hex(speeds[index]); hex(0.0f); hex(loco.m_angleOffset); hex(loco.m_offsetIncrement);
        hex(width); flag(loco.increasing); result(loco.step(speeds[index],0.0f)); std::puts("");
    }
}
int main() {
    const Case cases[] = {
        {"forward",60,0,0},
        {"backward",-60,0,0},
        {"lateral",0,0,60},
        {"vertical",0,90,0},
        {"vertical_contamination",60,90,0},
        {"mixed",30,0,-40,PI/4,PI/4},
        {"decreasing",60,0,0,0,0,0.1875f,0.0625f,1,false},
        {"near_limit",60,0,0,0,0,0.375f},
        {"reverse_second",-60,0,0,0,0,0.3125f},
        {"stationary",0,0,0},
        {"negative_heading",30,0,40,-PI/4,-PI/4},
        {"diagonal_reverse",-30,0,40,PI/4,PI/4},
        {"zero_width",60,0,0,0,0,0.1875f,0.0625f,0,true},
        {"negative_width",60,0,0,0,0,0.1875f,0.0625f,-1,true},
        {"zero_increment",60,0,0,0,0,0.1875f,0,1,true},
        {"negative_increment",60,0,0,0,0,0.1875f,-0.0625f,1,true},
        {"wrap_positive",60,0,0,0,PI,0.1875f},
        {"wrap_negative",-60,0,0,0,-PI,-0.1875f},
        {"rounding_a",0.1f,0,-0.2f,PI/6,PI/6},
        {"rounding_b",7.0f,0,-13.0f,PI/3,PI/3},
        {"rounding_c",123.456f,0,-54.321f,PI/4,PI/4},
        {"rounding_d",from_bits(0x3f800001),0,-from_bits(0x40000001),PI/4,PI/4}
    };
    for (const auto& c : cases) live(c);

    const Real limit = PI/8;
    const Real qnan = from_bits(0x7fc12345), inf=std::numeric_limits<Real>::infinity();
    scalar("zero_speed",0,0,0.1875f,0.0625f,1,true);
    scalar("negative_zero_speed",-0.0f,0,-0.0f,0.0625f,1,true);
    scalar("zero_width_retains_state",2,0.5f,0.375f,0.0625f,0,true);
    scalar("negative_zero_width",2,0.5f,0.375f,0.0625f,-0.0f,true);
    scalar("zero_increment_retained",2,0,0.1875f,0,1,true);
    scalar("negative_zero_increment",2,0,0.1875f,-0.0f,1,true);
    scalar("negative_increment",2,0,0.1875f,-0.0625f,1,true);
    scalar("negative_width",2,0,0.1875f,0.0625f,-1,true);
    scalar("positive_limit_equal",0,0,limit,0.0625f,1,true);
    scalar("positive_limit_below",0,0,nextafterf(limit,0),0.0625f,1,true);
    scalar("positive_limit_above",0,0,nextafterf(limit,inf),0.0625f,1,true);
    scalar("negative_limit_equal",0,0,-limit,0.0625f,1,false);
    scalar("negative_limit_inside",0,0,nextafterf(-limit,0),0.0625f,1,false);
    scalar("negative_limit_beyond",0,0,nextafterf(-limit,-inf),0.0625f,1,false);
    scalar("positive_overshoot",2,0,0.375f,0.0625f,1,true);
    scalar("negative_overshoot",2,0,-0.375f,0.0625f,1,false);
    scalar("reverse_increasing",-2,0,0.1875f,0.0625f,1,true);
    scalar("reverse_decreasing",-2,0,0.1875f,0.0625f,1,false);
    scalar("subnormal_increment",1,0,0,from_bits(1),1,true);
    scalar("nan_speed",qnan,0,0.1875f,0.0625f,1,true);
    scalar("nan_offset",2,0,qnan,0.0625f,1,true);
    scalar("nan_width",2,0,0.1875f,0.0625f,qnan,true);
    scalar("nan_increment",2,0,0.1875f,qnan,1,true);
    scalar("nan_desired",2,qnan,0.1875f,0.0625f,1,true);
    // Zero width returns before normalization, so infinity cannot cause a loop.
    scalar("zero_width_infinite_speed",inf,0.5f,0.375f,0.0625f,0,true);

    speed("forward",Coord3D{2,0,0},1,0);
    speed("backward",Coord3D{-2,0,0},1,0);
    speed("lateral",Coord3D{0,2,0},1,0);
    speed("vertical",Coord3D{0,0,3},1,0);
    speed("vertical_contamination",Coord3D{2,0,3},1,0);
    speed("component_products_not_dot",Coord3D{3,4,0},0.6f,0.8f);
    speed("component_products_reverse",Coord3D{-3,-4,0},0.6f,0.8f);
    speed("zero_dot_positive_magnitude",Coord3D{4,-3,0},0.6f,0.8f);
    speed("all_negative_zero",Coord3D{-0.0f,-0.0f,-0.0f},1,0);
    speed("subnormal_square_underflow",Coord3D{from_bits(1),0,0},1,0);
    speed("large_square_overflow",Coord3D{from_bits(0x7f7fffff),0,0},1,0);
    speed("nan_velocity",Coord3D{qnan,0,0},1,0);
    speed("infinite_forward",Coord3D{inf,0,0},1,0);

    normalize("positive_zero",0);
    normalize("negative_zero",-0.0f);
    normalize("positive_pi",PI);
    normalize("negative_pi",-PI);
    normalize("above_positive_pi",nextafterf(PI,inf));
    normalize("below_negative_pi",nextafterf(-PI,-inf));
    normalize("inside_negative_pi",nextafterf(-PI,0));
    normalize("multiple_turns",9*PI);
    normalize("quiet_nan",qnan);

    const Real reversal[]={2,-2,0,2,2,-2,-2};
    sequence("reversal",reversal,7,0.1875f,0.0625f,1,true);
    const Real oscillation[]={2,2,2,2,2,2,2,2,2,2,2,2};
    sequence("overshoot",oscillation,12,0.375f,0.0625f,1,true);
}
'''


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=DEFAULT_REPO)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--verify", type=Path)
    parser.add_argument("--compiler", default="g++")
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    source_data = {}
    for path, expected in PINS.items():
        data = (args.repo / path).read_bytes()
        if sha(data) != expected:
            raise RuntimeError("Original source pin mismatch: " + path)
        source_data[path] = data
    extracted = {}
    extraction_records = []
    for name, (path, first, last, expected) in BLOCKS.items():
        block = b"".join(source_data[path].splitlines(keepends=True)[first-1:last])
        if sha(block) != expected:
            raise RuntimeError("Original block pin mismatch: " + name)
        extracted[name] = block.decode()
        (out / (name + ".original.txt")).write_bytes(block)
        extraction_records.append({"name":name,"path":path,"first_line":first,
                                   "last_line":last,"sha256":sha(block),"bytes":len(block)})
    base = source_data[list(PINS)[4]].splitlines(keepends=True)
    pi, real = base[67].decode(), base[105].decode()
    if not pi.startswith("#define PI ") or "typedef float" not in real or "Real;" not in real:
        raise RuntimeError("Original Real/PI extraction mismatch")
    for name, block, line in (("pi_macro",pi,68),("real_typedef",real,106)):
        extraction_records.append({"name":name,"path":list(PINS)[4],"first_line":line,
                                   "last_line":line,"sha256":sha(block.encode()),"bytes":len(block.encode())})
        (out / (name + ".original.txt")).write_text(block)
    program = (PREFIX + pi + real + extracted["frame_conversions"] + SHIMS
               + extracted["forward_speed"] + extracted["normalize_angle"]
               + LOCOMOTOR_PREFIX + extracted["wander_width_block"] + LOCOMOTOR_SUFFIX)
    for name, block in extracted.items():
        if program.count(block) != 1:
            raise RuntimeError("Extracted original code not embedded exactly once: " + name)
    source = out / "infantry_wander_original.cpp"
    source.write_text(program)
    compiler = shutil.which(args.compiler)
    if compiler is None:
        raise RuntimeError("Compiler not found: " + args.compiler)
    compiler_path = Path(compiler).resolve()
    binary = out / "infantry_wander_original"
    flags = ["-std=c++17", "-O0", "-Werror=format", "-ffp-contract=off", "-fno-fast-math"]
    command = [str(compiler_path), *flags, str(source), "-o", str(binary)]
    subprocess.run(command, check=True, timeout=60)
    run = subprocess.run([str(binary)], capture_output=True, check=True, timeout=10)
    fixture = HEADER.encode() + run.stdout
    rows = [line.split() for line in fixture.decode().splitlines() if line and not line.startswith("#")]
    column_counts = {"live":20,"scalar":12,"sequence":13,"speed":8,"normalize":4}
    counts = {name:0 for name in column_counts}
    for row in rows:
        if len(row) != column_counts[row[0]]:
            raise RuntimeError("Unexpected fixture columns: " + " ".join(row))
        counts[row[0]] += 1
    (out / "infantry_wander_original.txt").write_bytes(fixture)
    if args.verify and args.verify.read_bytes() != fixture:
        raise RuntimeError("Frozen fixture differs from original-extracted output")
    version = subprocess.check_output([str(compiler_path),"--version"]).decode()
    metadata = {
        "revision":REVISION,"repository":str(args.repo.resolve()),
        "claim":"Executed extracted original arithmetic only; no original game or Rust public tick executed",
        "source_pins":PINS,"extractions":extraction_records,
        "generator_sha256":sha(Path(__file__).read_bytes()),
        "generated_cpp_sha256":sha(source.read_bytes()),
        "fixture_sha256":sha(fixture),"binary_sha256":sha(binary.read_bytes()),
        "compiler":{"path":str(compiler_path),"sha256":sha(compiler_path.read_bytes()),"version":version},
        "compile_command":command,"flags":flags,"row_counts":counts,
        "direction_contract":"Live dx=cosf(heading), dy=sinf(heading) supplied by harness; original Object/Thing matrix construction is not executed",
        "axis_contract":"Original XY=(host X,-host Z), original vertical Z=host Y",
        "raw_contract":"Convert each velocity component using original ConvertVelocityInSecsToFrames before original forward-speed body",
        "adapted_contract":"Original forward-speed body on host-unit components, then original ConvertVelocityInSecsToFrames on signed speed",
        "limits":["No constructor or RNG execution","No appearance/blocked/airborne/downhill dispatcher execution",
                  "No parser/admission, fixed-tick, steering, acceleration, integration, assets or rendering execution",
                  "Quiet NaN normalization executes original fallback with DEBUG_ASSERTCRASH disabled",
                  "Infinity is restricted to speed-only or width-zero controls; no nonterminating infinite-angle normalization"],
    }
    (out / "extraction.json").write_text(json.dumps(metadata,indent=2)+"\n")
    print(json.dumps({"output":str(out),"rows":counts,"fixture_sha256":sha(fixture)},indent=2))


if __name__ == "__main__":
    main()
