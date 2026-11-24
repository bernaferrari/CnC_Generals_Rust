#!/usr/bin/env python3
"""Compare original OCL forces, placement, player eligibility, and detonation with Rust."""

import argparse
import math
from pathlib import Path
import re
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[4]
WWMATH = ROOT / "GeneralsMD/Code/Libraries/Source/WWVegas/WWMath"
OCL_SOURCE = ROOT / "GeneralsMD/Code/GameEngine/Source/GameLogic/Object/ObjectCreationList.cpp"
PLAYER_SOURCE = ROOT / "GeneralsMD/Code/GameEngine/Source/Common/RTS/Player.cpp"
WEAPON_SOURCE = OCL_SOURCE.with_name("Weapon.cpp")
WEAPON_HEADER = ROOT / "GeneralsMD/Code/GameEngine/Include/GameLogic/Weapon.h"
MODEL_DRAW_SOURCE = (
    ROOT / "GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DModelDraw.cpp"
)
RUST_SOURCE = ROOT / "GeneralsRust/Code/GameEngine/GameLogic/src/object_creation_list/nuggets.rs"

CASES = [
    (0.0, 0.0, 10.0),
    (0.3, 1.2, 60.0),
    (1.8, -0.5, 120.125),
    (2.0, 1.57079632679, 50.0),
    (4.9, 0.7, 27.13),
    (6.2, 1.4, 101.7),
    (1.2, 0.8, 0.0),
]


def definition(text: str, signature: str) -> str:
    # The selected top-level functions have an unindented closing brace.
    match = re.search(signature, text)
    if match is None:
        raise ValueError(f"Missing source definition {signature}")
    end = text.index("\n}", match.end()) + 2
    return text[match.start():end]


def cpp_source() -> str:
    header = (WWMATH / "matrix3d.h").read_text()
    pi = re.search(r"^#define\s+WWMATH_PI\s+(\d+\.\d+f)\s*$",
                   (WWMATH / "wwmath.h").read_text(), re.MULTILINE)
    if pi is None:
        raise ValueError("Missing WWMATH_PI")
    matrix = "\n".join(definition(header, signature) for signature in (
        r"WWINLINE void Matrix3D::Rotate_Y\(float theta\)",
        r"WWINLINE void Matrix3D::Rotate_Z\(float theta\)",
        r"WWINLINE void\s+Matrix3D::Scale\(float scale\)",
    ))
    getter = re.search(r"WWINLINE Vector3 Get_X_Vector\(\) const \{[^\n]+\}", header)
    if getter is None:
        raise ValueError("Missing Matrix3D::Get_X_Vector")
    force = definition(OCL_SOURCE.read_text(), r"static void calcRandomForce\(")
    calls = "\n".join(
        f"    run({angle}f, {pitch}f, {magnitude}f);"
        for angle, pitch, magnitude in CASES
    )
    return f"""
#include <cassert>
#include <cmath>
#include <cstdio>
#define WWINLINE inline
constexpr float PI = {pi.group(1)};
using Real = float;
struct Coord3D {{ float x, y, z; }};
struct Vector3 {{
    float X, Y, Z;
    Vector3(float x, float y, float z) : X(x), Y(y), Z(z) {{}}
}};
class Matrix3D {{
    float Row[3][4] {{{{1,0,0,0}},{{0,1,0,0}},{{0,0,1,0}}}};
public:
    explicit Matrix3D(int) {{}}
    void Scale(float);
    void Rotate_Y(float);
    void Rotate_Z(float);
    {getter.group()}
}};
{matrix}
float draw_values[3];
int draw_index;
Real GameLogicRandomValueReal(Real lo, Real hi) {{
    assert(draw_index < 3);
    const float bounds[3][2] = {{{{0,2*PI}},{{-2,2}},{{0,200}}}};
    assert(lo == bounds[draw_index][0] && hi == bounds[draw_index][1]);
    return draw_values[draw_index++];
}}
{force}
void run(float angle, float pitch, float magnitude) {{
    draw_values[0] = angle;
    draw_values[1] = pitch;
    draw_values[2] = magnitude;
    draw_index = 0;
    Coord3D result;
    calcRandomForce(0.0f, 200.0f, -2.0f, 2.0f, &result);
    assert(draw_index == 3);
    std::printf("%.9g %.9g %.9g\\n", result.x, result.y, result.z);
}}
int main() {{
{calls}
}}
"""


def rust_source() -> str:
    source = RUST_SOURCE.read_text()
    vector = definition(source, r"pub fn calc_random_force_xyz\(")
    force = definition(source, r"(?:pub\(super\) )?fn calc_random_force_with\(")
    calls = "\n".join(
        f"    run([{angle}, {pitch}, {magnitude}]);"
        for angle, pitch, magnitude in CASES
    )
    return f"""
use std::f32::consts::PI;
type Real = f32;
pub struct Coord3D(f32, f32, f32);
impl Coord3D {{
    fn new(x: f32, y: f32, z: f32) -> Self {{ Self(x, y, z) }}
}}
{vector}
mod ocl {{
    use super::*;
    {force}
}}
fn run(values: [f32; 3]) {{
    let bounds = [(0.0, 2.0 * PI), (-2.0, 2.0), (0.0, 200.0)];
    let mut draws = bounds.into_iter().zip(values);
    let result = ocl::calc_random_force_with(|lo, hi| {{
        let (request, value) = draws.next().expect("extra force draw");
        assert_eq!((lo, hi), request);
        value
    }}, 0.0, 200.0, -2.0, 2.0);
    assert!(draws.next().is_none(), "missing force draw");
    println!("{{:.9e}} {{:.9e}} {{:.9e}}", result.0, result.1, result.2);
}}
fn main() {{
{calls}
}}
"""


def observed(binary: Path) -> list[tuple[float, float, float]]:
    output = subprocess.check_output([binary], text=True)
    return [tuple(map(float, line.split())) for line in output.splitlines()]


def verify_object_placement(cxx: str, temp: Path) -> None:
    original = OCL_SOURCE.read_text()
    rotate = definition(
        (WWMATH / "matrix3d.cpp").read_text(),
        r"Vector3 Matrix3D::Rotate_Vector\(const Vector3 &vect\) const",
    )
    transform = definition(original, r"static void adjustVector\(")
    start = original.index("Coord3D offset = m_offset;")
    end = original.index("if (!m_particleSysName.isEmpty())", start)
    offset = original[start:end]
    start = original.index("if (mtx)", original.index("if( BitTest( m_disposition, LIKE_EXISTING ) )", end))
    end = original.index("if (sourceObj && sourceObj->isAboveTerrain())", start)
    like_existing = original[start:end]
    cpp = temp / "object_basis.cpp"
    binary = temp / "object_basis"
    cpp.write_text(
        r"""
#include <cstdio>
using Real = float;
struct Coord3D { Real x, y, z; };
struct Vector3 {
    Real X = 0, Y = 0, Z = 0;
    Vector3() = default;
    Vector3(Real x, Real y, Real z) : X(x), Y(y), Z(z) {}
    Real operator[](int index) const { return index == 0 ? X : index == 1 ? Y : Z; }
};
struct Matrix3D {
    Real Row[3][4]{{0,1,0,20},{0,0,1,30},{1,0,0,4}};
    Vector3 Rotate_Vector(const Vector3&) const;
};
struct Object {
    Matrix3D transform;
    void setTransformMatrix(const Matrix3D* matrix) { transform = *matrix; }
    void setOrientation(Real) {}
    void setPosition(const Coord3D* pos) {
        transform.Row[0][3] = pos->x;
        transform.Row[1][3] = pos->y;
        transform.Row[2][3] = pos->z;
    }
};
"""
        + rotate + "\n" + transform + r"""
int main() {
    Matrix3D matrix;
    const Matrix3D* mtx = &matrix;
    const Coord3D start{20,30,4};
    const Coord3D* pos = &start;
    const Coord3D m_offset{1,2,3};
    const Real orientation = 0;
    Object child;
    Object* obj = &child;
"""
        + offset + like_existing + r"""
    // Emit the pose in the host Y-up basis with one row per column.
    constexpr int host_to_original[3]{0,2,1};
    for (int col : host_to_original)
        std::printf("%.0f %.0f %.0f\n", child.transform.Row[0][col],
                    child.transform.Row[2][col], child.transform.Row[1][col]);
    std::printf("%.0f %.0f %.0f\n", child.transform.Row[0][3],
                child.transform.Row[2][3], child.transform.Row[1][3]);
}
"""
    )
    subprocess.run([cxx, "-std=c++17", cpp, "-o", binary], check=True)
    observed = subprocess.check_output([binary], text=True)
    fixture = ROOT / (
        "GeneralsRust/Code/Main/src/game_logic/world_combat/ocl_and_scud/object_basis.trace"
    )
    if observed != fixture.read_text():
        raise RuntimeError(f"Original OCL Object placement differs from {fixture.name}:\n{observed}")
    print("Original OCL matched the full Object basis and offset")


def verify_detonation_rank(cxx: str, temp: Path) -> None:
    source = WEAPON_SOURCE.read_text()
    rank_read = "VeterancyLevel v = sourceObj->getVeterancyLevel();"
    fx_start = source.rindex(rank_read, 0, source.index(
        "const FXList* fx = isProjectileDetonation ? getProjectileDetonateFX"
    ))
    fx = source[fx_start:source.index(";", source.index("getFireFX(v);", fx_start)) + 1]
    ocl_start = source.index("if( sourceObj )", source.index("// Now do the FireOCL"))
    ocl = source[ocl_start:source.index("\n\t}", ocl_start) + len("\n\t}")]
    getters = re.findall(
        r"^\s*inline const (?:FXList|ObjectCreationList)\* "
        r"(?:getFireFX|getProjectileDetonateFX|getFireOCL|getProjectileDetonationOCL)"
        r"\(VeterancyLevel v\) const \{[^\n]+\}",
        WEAPON_HEADER.read_text(),
        re.MULTILINE,
    )
    if len(getters) != 4:
        raise ValueError("Missing ranked weapon effect lookup")
    cpp = temp / "detonation_rank.cpp"
    binary = temp / "detonation_rank"
    cpp.write_text(r"""
#include <cassert>
#include <cstdio>

enum VeterancyLevel { Regular, Veteran, Elite, Heroic };
struct Object {
    VeterancyLevel rank;
    VeterancyLevel getVeterancyLevel() const { return rank; }
};
struct FXList {};
struct ObjectCreationList {
    static void create(const ObjectCreationList*, Object* source, void*) {
        assert(source->rank == Veteran);
        std::puts("OCL_RankedImpact");
    }
};
struct WeaponTemplate {
    FXList veteran_fx;
    ObjectCreationList veteran_ocl;
    const FXList* m_fireFXs[4]{};
    const FXList* m_projectileDetonateFXs[4]{nullptr, &veteran_fx};
    const ObjectCreationList* m_fireOCLs[4]{};
    const ObjectCreationList* m_projectileDetonationOCLs[4]{nullptr, &veteran_ocl};
"""
        + "\n".join(getters) + r"""
    void detonate(Object* sourceObj, VeterancyLevel rank_after_fx) {
        const bool isProjectileDetonation = true;
        {
"""
        + fx + r"""
            std::printf("%s ", fx == &veteran_fx ? "FX_RankedImpact" : "None");
            sourceObj->rank = rank_after_fx;
        }
"""
        + ocl + r"""
        if (sourceObj->rank != Veteran)
            std::puts("None");
    }
};
int main() {
    WeaponTemplate weapon;
    Object projectile{Veteran};
    weapon.detonate(&projectile, Veteran);
    weapon.detonate(&projectile, Elite);
    weapon.detonate(&projectile, Elite);
}
""")
    subprocess.run([cxx, "-std=c++17", cpp, "-o", binary], check=True)
    output = subprocess.check_output([binary], text=True)
    expected = "FX_RankedImpact OCL_RankedImpact\nFX_RankedImpact None\nNone None\n"
    if output != expected:
        raise RuntimeError(f"Original weapon's detonation rank selection differs:\n{output}")
    print("Original detonation uses its actor's rank separately for FX and OCL")


def verify_detonation_fx_position(cxx: str, temp: Path) -> None:
    source = WEAPON_SOURCE.read_text()
    start = source.index("if (handled == false && fx != NULL)")
    end = source.index("\n\t\t}", start) + len("\n\t\t}")
    fallback = source[start:end]
    draw_source = MODEL_DRAW_SOURCE.read_text()
    draw_start = draw_source.index("if( ! m_renderObject->Is_Hidden()", draw_source.index(
        "Bool W3DModelDraw::handleWeaponFireFX"
    ))
    draw_end = draw_source.index("handled = true;", draw_start) + len("handled = true;")
    draw_fx = draw_source[draw_start:draw_end]
    cpp = temp / "detonation_fx_position.cpp"
    binary = temp / "detonation_fx_position"
    cpp.write_text(r"""
#include <cstdio>

struct Coord3D { int x, y, z; };
struct Matrix3D {
    // Local axes of the tilting projectile, in original Z-up coordinates.
    Coord3D x{0,0,1}, y{1,0,0}, z{0,1,0};
    Coord3D translation{};
    int Get_X_Translation() const { return translation.x; }
    int Get_Y_Translation() const { return translation.y; }
    int Get_Z_Translation() const { return translation.z; }
};
struct Drawable {
    Coord3D position{10,12,4};
    Matrix3D pose;
    const Coord3D* getPosition() const { return &position; }
    const Matrix3D* getTransformMatrix() const { return &pose; }
};
struct Object {
    Drawable drawable;
    const Drawable* getDrawable() const { return &drawable; }
    const Coord3D* getPosition() const { return drawable.getPosition(); }
    const Matrix3D* getTransformMatrix() const { return drawable.getTransformMatrix(); }
};
struct FXList {
    static void doFXPos(const FXList*, const Coord3D* pos, const Matrix3D* mtx,
                        int speed, const Coord3D* target, int radius) {
        // Witness a particle with authored X:1 Y:2 Z:3 offset.
        std::printf("%d %d %d %d %d %d %d %d\n",
            pos->x + mtx->x.x + 2*mtx->y.x + 3*mtx->z.x,
            pos->y + mtx->x.y + 2*mtx->y.y + 3*mtx->z.y,
            pos->z + mtx->x.z + 2*mtx->y.z + 3*mtx->z.z,
            target->x, target->y, target->z, speed, radius);
    }
};
struct WeaponTemplate {
    bool contact;
    bool isContactWeapon() const { return contact; }
    int getWeaponSpeed() const { return 13; }
    int getPrimaryDamageRadius(int bonus) const { return 20 * bonus; }
    void detonate(const Object* sourceObj) const {
        const Coord3D targetPos{40,32,4};
        const bool handled = false;
        const FXList selected;
        const auto* fx = &selected;
        const int bonus = 2;
"""
        + fallback + r"""
    }
};
struct RenderObject {
    bool hidden;
    bool Is_Hidden() const { return hidden; }
    Matrix3D Get_Bone_Transform(int) const {
        return {{1,0,0}, {0,1,0}, {0,0,1}, {80,60,9}};
    }
};
bool fire_from_bone(const Object* logicObject, const RenderObject* m_renderObject) {
    const FXList selected;
    const auto* fxl = &selected;
    const int weaponSpeed = 13, damageRadius = 40;
    const Coord3D position{40,32,4};
    const auto* victimPos = &position;
    struct { int m_fxBone = 1; } info;
    bool handled = false;
"""
        + draw_fx + r"""
    return handled;
}
int main() {
    const Object projectile;
    WeaponTemplate{false}.detonate(&projectile);
    WeaponTemplate{true}.detonate(&projectile);
    const RenderObject visible{false}, hidden{true};
    if (!fire_from_bone(&projectile, &visible) || !fire_from_bone(&projectile, &hidden))
        return 1;
}
""")
    subprocess.run([cxx, "-std=c++17", cpp, "-o", binary], check=True)
    output = subprocess.check_output([binary], text=True)
    expected = (
        "12 15 5 40 32 4 13 40\n42 35 5 40 32 4 13 40\n"
        "81 62 12 40 32 4 13 40\n12 15 5 40 32 4 13 40\n"
    )
    if output != expected:
        raise RuntimeError(f"Original weapon's detonation FX placement differs:\n{output}")
    print("Original detonation tries the installed FX bone before the positional fallback")



def verify_detonation_draw_order(cxx: str, temp: Path) -> None:
    drawable_source = (ROOT / "GeneralsMD/Code/GameEngine/Source/GameClient/Drawable.cpp").read_text()
    model_source = MODEL_DRAW_SOURCE.read_text()
    drawable_start = drawable_source.index("Bool Drawable::handleWeaponFireFX(")
    drawable_end = drawable_source.index("\n}", drawable_start) + 2
    model_start = model_source.index("Bool W3DModelDraw::handleWeaponFireFX(")
    model_end = model_source.index("\n}", model_start) + 2
    cpp = temp / "detonation_draw_order.cpp"
    binary = temp / "detonation_draw_order"
    cpp.write_text(r"""
#include <cmath>
#include <cstdlib>
#include <string>
#include <vector>
#define DEBUG_ASSERTCRASH(...)
#define DEBUG_LOG(...)
using Bool = bool;
using Int = int;
using Real = float;
using WeaponSlotType = int;
constexpr Real PI = 3.14159265358979323846f;
using std::cos;
using std::sin;
float Cos(float x) { return cos(x); }
float Sin(float x) { return sin(x); }
struct Coord3D { float x, y, z; };
struct Matrix3D {
    float Get_X_Translation() const { return 0.f; }
    float Get_Y_Translation() const { return 0.f; }
    float Get_Z_Translation() const { return 0.f; }
};
struct Object {
    float getOrientation() const { return 0.f; }
    const Matrix3D* getTransformMatrix() const { return nullptr; }
    const Coord3D* getPosition() const { std::abort(); }
};
std::string trace;
struct FXList {
    static void doFXPos(const FXList*, const Coord3D*, const Matrix3D*,
                        Real, const Coord3D*, Real) { trace += 'F'; }
};
struct ObjectDrawInterface {
    virtual bool handleWeaponFireFX(int, int, const FXList*, Real, const Coord3D*, Real) = 0;
};
struct DrawModule {
    ObjectDrawInterface* draw;
    ObjectDrawInterface* getObjectDrawInterface() const { return draw; }
};
struct Drawable {
    struct LocoInfo { Real m_accelerationPitchRate{}, m_accelerationRollRate{}; };
    LocoInfo* m_locoInfo;
    Object obj;
    DrawModule* modules[4]{};
    const Object* getObject() const { return &obj; }
    DrawModule** getDrawModules() { return modules; }
    bool handleWeaponFireFX(int, int, const FXList*, Real, Real, Real, const Coord3D*, Real);
};
struct ModelConditionInfo {
    enum { BARRELS_VALID = 1 };
    struct WeaponBarrelInfo {
        int m_fxBone, m_recoilBone, m_muzzleFlashBone;
        char tag;
        void setMuzzleFlashHidden(void*, bool hidden) const {
            if (hidden) std::abort();
            trace += tag;
        }
    };
    using WeaponBarrelInfoVec = std::vector<WeaponBarrelInfo>;
    int m_validStuff = BARRELS_VALID;
    WeaponBarrelInfoVec m_weaponBarrelInfoVec[1];
};
struct WeaponRecoilInfo {
    enum { RECOIL_START = 1 };
    int m_state = 0;
    float m_recoilRate = 0;
};
struct W3DModelDraw : ObjectDrawInterface {
    struct RenderObject {
        bool Is_Hidden() const { return false; }
        Matrix3D Get_Bone_Transform(int) const { return {}; }
    } render;
    RenderObject* m_renderObject = &render;
    struct Data { float m_initialRecoil = 2.f; } data;
    ModelConditionInfo state;
    const ModelConditionInfo* m_curState = &state;
    WeaponRecoilInfo m_weaponRecoilInfoVec[1][1];
    Drawable* owner;
    const Drawable* getDrawable() const { return owner; }
    const Data* getW3DModelDrawModuleData() const { return &data; }
    W3DModelDraw(Drawable* owner, int fxBone, char tag) : owner(owner) {
        state.m_weaponBarrelInfoVec[0].push_back({fxBone, 1, 1, tag});
    }
    bool handleWeaponFireFX(int, int, const FXList*, Real, const Coord3D*, Real) override;
};
""" + drawable_source[drawable_start:drawable_end]
        + model_source[model_start:model_end] + r"""
int main() {
    Drawable::LocoInfo loco;
    Drawable drawable{&loco};
    W3DModelDraw first(&drawable, 0, 'a'), second(&drawable, 1, 'b'), third(&drawable, 1, 'c');
    DrawModule a{&first}, b{&second}, c{&third};
    drawable.modules[0] = &a;
    drawable.modules[1] = &b;
    drawable.modules[2] = &c;
    FXList fx;
    if (!drawable.handleWeaponFireFX(0, 0, &fx, 0, 2, 0, nullptr, 0)
        || trace != "aFb" || third.m_weaponRecoilInfoVec[0][0].m_state != 0
        || loco.m_accelerationPitchRate != -2.f) return 1;
    trace.clear();
    if (drawable.handleWeaponFireFX(0, 0, nullptr, 0, 2, 0, nullptr, 0)
        || trace != "abc" || third.m_weaponRecoilInfoVec[0][0].m_state != WeaponRecoilInfo::RECOIL_START
        || third.m_weaponRecoilInfoVec[0][0].m_recoilRate != 2.f
        || loco.m_accelerationPitchRate != -4.f) return 2;
}
""")
    subprocess.run([cxx, "-std=c++17", cpp, "-o", binary], check=True)
    subprocess.run([binary], check=True)
    print("Original Drawable starts recoil across the Draw chain with and without FX")


def verify_recoil_duration(cxx: str, temp: Path) -> None:
    source = MODEL_DRAW_SOURCE.read_text()
    method = source.index("void W3DModelDraw::handleClientRecoil()")
    visibility = source[source.index("Bool hidden =", method):].splitlines()[0]
    start = source.index("switch (recoils[i].m_state )", method)
    end = source.index("Matrix3D gunXfrm;", start)
    recoil_step = source[start:end]
    muzzle_only = source[source.index("recoils[i].m_state = WeaponRecoilInfo::IDLE;", end):].splitlines()[0]
    cpp = temp / "recoil_duration.cpp"
    binary = temp / "recoil_duration"
    cpp.write_text(r"""
#include <cmath>
#include <cstdio>
using Bool = bool;
struct WeaponRecoilInfo {
    enum { IDLE, RECOIL_START, RECOIL, SETTLE };
    int m_state = RECOIL_START;
    float m_shift = 0.f, m_recoilRate = 2.f;
};
struct Data {
    float m_recoilDamping = 0.4f, m_maxRecoil = 3.f, m_recoilSettle = 0.065f;
};
void step(WeaponRecoilInfo* recoils, bool recoilBone) {
    const Data data;
    const auto* d = &data;
    const int i = 0;
    const float TINY_RECOIL = 0.01f;
"""
        + visibility + "\nif (recoilBone) {\n" + recoil_step
        + "\n} else {\n" + muzzle_only + "\n}\n"
        + r"""
    std::printf("%d %d %.3f %.3f\n",
        !hidden, recoils[i].m_state, recoils[i].m_shift, recoils[i].m_recoilRate);
}
int main() {
    WeaponRecoilInfo recoil, muzzleOnly;
    for (int n = 0; n < 4; ++n) step(&recoil, true);
    step(&muzzleOnly, false);
    step(&muzzleOnly, false);
}
""")
    subprocess.run([cxx, "-std=c++17", cpp, "-o", binary], check=True)
    output = subprocess.check_output([binary], text=True)
    expected = (
        "1 1 2.000 0.800\n1 1 2.800 0.320\n1 3 3.000 0.128\n0 3 2.935 0.128\n"
        "1 0 0.000 2.000\n0 0 0.000 2.000\n"
    )
    if output != expected:
        raise RuntimeError(f"Original muzzle and barrel recoil trace differs:\n{output}")
    print("Original recoil barrel flashes across three draws and muzzle-only barrel across one")


def verify_ocl_player_eligibility(cxx: str, temp: Path) -> None:
    original = OCL_SOURCE.read_text()
    start = original.index("if (m_requiresLivePlayer &&")
    stop = original.index("return NULL;", start) + len("return NULL;")
    guard = original[start:stop]
    is_player_active = definition(
        PLAYER_SOURCE.read_text(), r"Bool Player::isPlayerActive\(void\) const"
    )
    cpp = temp / "ocl_player_eligibility.cpp"
    binary = temp / "ocl_player_eligibility"
    cpp.write_text(
        """
#include <cassert>
#include <cstddef>
#include <initializer_list>
using Bool = bool;
struct Player {
    bool m_observer;
    bool m_isPlayerDead;
    Bool isPlayerActive() const;
};
struct Object {
    const Player* owner;
    const Player* getControllingPlayer() const { return owner; }
};
"""
        + is_player_active
        + """
const Object* createObject(bool m_requiresLivePlayer, const Object* sourceObj) {
"""
        + guard
        + """
    static const Object created{nullptr};
    return &created;
}
int main() {
    const Player active{false, false}, defeated{false, true}, observer{true, false};
    const Object activeSource{&active}, defeatedSource{&defeated},
        observerSource{&observer}, unownedSource{nullptr};
    assert(createObject(true, &activeSource));
    for (const Object* sourceObj : {&defeatedSource, &observerSource, &unownedSource}) {
        assert(!createObject(true, sourceObj));
        assert(createObject(false, sourceObj));
    }
    assert(!createObject(true, nullptr));
}
"""
    )
    subprocess.run([cxx, "-std=c++17", cpp, "-o", binary], check=True)
    subprocess.run([binary], check=True)
    print("Original OCL checked the controlling player's active state for gated entries")


def verify_put_in_container(cxx: str, temp: Path) -> None:
    original = OCL_SOURCE.read_text()
    start = original.index("Object* container = NULL;", original.index("if (m_requiresLivePlayer &&"))
    child_end = original.index("// if we want the objects being created", start)
    child_definitions = original[start:child_end]
    finish_child_start = original.index("doStuffToObj( debris, m_names[pick], pos", child_end)
    finish_child = original[finish_child_start:original.index(";", finish_child_start) + 1]
    finish_container_start = original.index("if (container)\n\t\t\tdoStuffToObj", child_end)
    end = original.index("return firstObject;", finish_container_start) + len("return firstObject;")
    finish_container = original[finish_container_start:end]

    cpp = temp / "ocl_container.cpp"
    binary = temp / "ocl_container"
    cpp.write_text(
        r"""
#include <cassert>
#include <cstddef>
#include <deque>
#include <string>
#include <vector>
using Int = int;
using PathfindLayerEnum = int;
constexpr PathfindLayerEnum LAYER_GROUND = 0;
#define DEBUG_CRASH(x) assert(false)
#define DEBUG_ASSERTCRASH(p, x) assert(p)
struct AsciiString : std::string {
    using std::string::string;
    bool isEmpty() const { return empty(); }
    static const AsciiString TheEmptyString;
};
const AsciiString AsciiString::TheEmptyString;
struct ThingTemplate { char mark; };
struct Object {
    char mark;
    const Object* producer = nullptr;
    Object* container = nullptr;
    std::vector<Object*> riders;
    void setProducer(const Object* value) { producer = value; }
    int getLayer() const { return LAYER_GROUND; }
    void setLayer(int) {}
    Object* getContain() { return mark == 'C' ? this : nullptr; }
    bool isValidContainerFor(const Object*, bool) const { return riders.empty(); }
    void addToContain(Object* rider) {
        riders.push_back(rider);
        rider->container = this;
    }
};
std::string events;
struct Factory {
    ThingTemplate canopy{'C'}, rider{'R'};
    std::deque<Object> objects;
    const ThingTemplate* findTemplate(const AsciiString& name) const {
        return name == "Canopy" ? &canopy : name == "Rider" ? &rider : nullptr;
    }
    Object* newObject(const ThingTemplate* tmpl, int) {
        events += tmpl->mark;
        objects.push_back(Object{tmpl->mark});
        return &objects.back();
    }
} factory, *TheThingFactory = &factory;
struct Lod { bool isDebrisSkipped() { return false; } } lod, *TheGameLODManager = &lod;
int GameLogicRandomValue(int, int) { events += '*'; return 0; }
void doStuffToObj(Object* obj, const AsciiString&, int, int, int, const Object* source, int) {
    assert(obj->producer == source);
    events += obj->mark == 'C' ? 'c' : obj->container ? '+' : '-';
}
Object* reallyCreate(const Object* sourceObj, AsciiString m_putInContainer) {
    constexpr bool m_nameAreObjects = true, m_preserveLayer = true;
    constexpr int m_debrisToGenerate = 2, debrisOwner = 0;
    constexpr int pos = 0, mtx = 0, orientation = 0, lifetimeFrames = 0;
    const ThingTemplate* debrisTemplate = nullptr;
    const std::vector<AsciiString> m_names{"Rider"};
"""
        + child_definitions
        + finish_child
        + "\n}\n"
        + finish_container
        + r"""
}
int main() {
    Object source{'S'};
    assert(reallyCreate(&source, "Canopy")->mark == 'C');
    assert(events == "C*R+*R-c");
    events.clear();
    assert(reallyCreate(&source, "Missing")->mark == 'R');
    assert(events == "*R-*R-");
}
"""
    )
    subprocess.run([cxx, "-std=c++17", cpp, "-o", binary], check=True)
    subprocess.run([binary], check=True)
    print("Original OCL creates the container first and initializes it after both riders")


def verify_eject_pilot(cxx: str, temp: Path) -> None:
    source = (OCL_SOURCE.parent / "Die/EjectPilotDie.cpp").read_text()
    eject = definition(source, r"void EjectPilotDie::ejectPilot\(")
    die = definition(source, r"void EjectPilotDie::onDie\(")
    cpp = temp / "eject_pilot.cpp"
    binary = temp / "eject_pilot"
    cpp.write_text(
        r"""
#include <cassert>
#include <cstring>
#include <string>
std::string events;
struct AudioEventRTS {
    char cue;
    int player = -1;
    void setPosition(const int* position) { assert(*position == 50); }
    void setPlayerIndex(int index) { player = index; }
};
struct Template {
    const AudioEventRTS* getPerUnitSound(const char* slot) const {
        static const AudioEventRTS voice{'V'}, sound{'S'};
        return std::strcmp(slot, "VoiceEject") == 0 ? &voice : &sound;
    }
};
struct Player { int getPlayerIndex() const { return 2; } };
struct Object {
    int position = 50;
    bool above = false;
    Template model;
    Player player;
    bool isSignificantlyAboveTerrain() const { return above; }
    const int* getPosition() const { return &position; }
    const Template* getTemplate() const { return &model; }
    const Player* getControllingPlayer() const { return &player; }
} victim, attacker;
struct ObjectCreationList {
    char name;
    static void create(const ObjectCreationList* list, const Object* main, const Object* other) {
        assert(main == &victim && other == &attacker);
        events += list->name;
    }
} air{'A'}, ground{'G'};
struct EjectPilotDieModuleData {
    const ObjectCreationList* m_oclInAir = &air;
    const ObjectCreationList* m_oclOnGround = &ground;
};
struct DamageInfo { struct { int m_sourceID; } in; };
struct GameLogic {
    Object* findObjectByID(int id) const { assert(id == 7); return &attacker; }
} logic, *TheGameLogic = &logic;
struct Audio {
    void addAudioEvent(const AudioEventRTS* cue) {
        assert(cue->player == (cue->cue == 'V' ? 2 : -1));
        events += cue->cue;
    }
} audio, *TheAudio = &audio;
struct EjectPilotDie {
    EjectPilotDieModuleData data;
    bool applicable = true;
    bool isDieApplicable(const DamageInfo*) const { return applicable; }
    Object* getObject() const { return &victim; }
    const EjectPilotDieModuleData* getEjectPilotDieModuleData() const { return &data; }
    static void ejectPilot(const ObjectCreationList*, const Object*, const Object*);
    void onDie(const DamageInfo* info);
};
"""
        + eject + "\n"
        + die
        + r"""
int main() {
    EjectPilotDie module;
    DamageInfo death{{7}};
    module.onDie(&death);
    victim.above = true;
    module.onDie(&death);
    module.data.m_oclInAir = nullptr;
    module.onDie(&death);
    module.applicable = false;
    victim.above = false;
    module.onDie(&death);
    assert(events == "GVSAVS");
}
"""
    )
    subprocess.run([cxx, "-std=c++17", cpp, "-o", binary], check=True)
    subprocess.run([binary], check=True)
    print("Original pilot death selects its authored list, then queues voice and sound")


def verify_projectile_attribution(cxx: str, temp: Path) -> None:
    weapon = WEAPON_SOURCE.read_text()
    start = weapon.index("Player *owningPlayer = sourceObj->getControllingPlayer();")
    launch = weapon[start:weapon.index("//If the player has battle plans", start)]
    ocl = OCL_SOURCE.read_text()
    start = ocl.index("Team *debrisOwner = ThePlayerList->getNeutralPlayer()")
    controller = ocl[start:ocl.index("Object* container = NULL;", start)]
    missile = OCL_SOURCE.parent / "Update/AIUpdate/MissileAIUpdate.cpp"
    detonate = definition(missile.read_text(), r"void MissileAIUpdate::detonate\(\)")
    cpp = temp / "projectile_attribution.cpp"
    binary = temp / "projectile_attribution"
    cpp.write_text(
        r"""
#include <cassert>
#include <string>
struct Player;
struct Team { Player* controller; };
struct Player {
    Team team{this};
    Team* getDefaultTeam() { return &team; }
} owner, captor, neutral;
enum { DAMAGE_UNRESISTABLE, DEATH_DETONATED, INVALID_ID, KILL_SELF };
struct DamageInfo {
    struct { int m_damageType, m_deathType, m_sourceID, m_amount; } in;
};
std::string events;
struct Object {
    Team* team;
    const Object* producer;
    Player* getControllingPlayer() const { return team->controller; }
    void setProducer(const Object* value) { producer = value; }
    int getPosition() const { return 50; }
    Object* getBodyModule() { return this; }
    int getMaxHealth() const { return 3; }
    void attemptDamage(DamageInfo* damage) {
        assert(team == &owner.team);
        assert(damage->in.m_deathType == DEATH_DETONATED);
        events += 'D';
    }
    struct Drawable { void setDrawableHidden(bool) {} };
    Drawable* getDrawable() { return nullptr; }
    void setStatus(int) {}
};
struct Factory {
    Object* newObject(int, Team* team) { return new Object{team, nullptr}; }
} factory, *TheThingFactory = &factory;
int getProjectileTemplate() { return 0; }
struct PlayerList { Player* getNeutralPlayer() { return &neutral; } } players, *ThePlayerList = &players;
struct Weapon { bool getDieOnDetonate() const { return true; } } warhead;
struct WeaponStore {
    void handleProjectileDetonation(Weapon*, Object* source, int, int, bool) {
        assert(source->getControllingPlayer() == &owner);
        events += 'W';
    }
} store, *TheWeaponStore = &store;
#define MAKE_OBJECT_STATUS_MASK(x) 0
struct MissileAIUpdate {
    Object* projectile;
    Weapon* m_detonationWeaponTmpl = &warhead;
    int m_extraBonusFlags = 0;
    bool m_noDamage = false;
    Object* getObject() { return projectile; }
    void switchToState(int) {}
    void detonate();
};
Object* admit(Object* sourceObj) {
"""
        + launch + "return projectile;\n}\n"
        + "Team* childOwner(const Object* sourceObj) {\n" + controller + "return debrisOwner;\n}\n"
        + detonate
        + r"""
int main() {
    Object launcher{&owner.team, nullptr};
    Object* projectile = admit(&launcher);
    launcher.team = &captor.team;
    assert(projectile->producer == &launcher);
    assert(childOwner(projectile) == &owner.team);
    MissileAIUpdate{projectile}.detonate();
    assert(events == "WD");
    delete projectile;
}
"""
    )
    subprocess.run([cxx, "-std=c++17", cpp, "-o", binary], check=True)
    subprocess.run([binary], check=True)
    print("Original projectile keeps its launch controller and damages the warhead victim before its own Die callbacks")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cxx", default="c++")
    parser.add_argument("--rustc", default="rustc")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="ocl-force-parity-") as temp:
        temp = Path(temp)
        cpp = temp / "original.cpp"
        rust = temp / "ported.rs"
        cpp.write_text(cpp_source())
        rust.write_text(rust_source())
        originals, port = temp / "original", temp / "ported"
        subprocess.run([args.cxx, "-std=c++17", "-ffp-contract=off", cpp, "-o", originals],
                       check=True)
        subprocess.run([args.rustc, "--edition=2024", rust, "-o", port], check=True)
        original, ported = observed(originals), observed(port)
        verify_object_placement(args.cxx, temp)
        verify_ocl_player_eligibility(args.cxx, temp)
        verify_put_in_container(args.cxx, temp)
        verify_eject_pilot(args.cxx, temp)
        verify_projectile_attribution(args.cxx, temp)
        verify_detonation_rank(args.cxx, temp)
        verify_detonation_fx_position(args.cxx, temp)
        verify_detonation_draw_order(args.cxx, temp)
        verify_recoil_duration(args.cxx, temp)
    if len(original) != len(CASES) or len(ported) != len(CASES):
        raise RuntimeError("Unexpected vector count")
    for scenario, expected, actual in zip(CASES, original, ported, strict=True):
        # The Rust scalar products group the products differently from the
        # original scaled matrix, so allow floating-point rounding differences.
        if not all(math.isclose(left, right, rel_tol=2e-7, abs_tol=1e-7)
                   for left, right in zip(expected, actual, strict=True)):
            raise RuntimeError(f"{scenario} C++ {expected} differs from Rust {actual}")
    print(f"Original C++ and Rust matched force draw order and {len(CASES)} vectors")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
