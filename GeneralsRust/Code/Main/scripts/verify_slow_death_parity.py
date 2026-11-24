#!/usr/bin/env python3
"""Replay the original ordinary SlowDeath update against the Rust test fixture."""

import argparse
from pathlib import Path
import subprocess
import tempfile

from verify_ocl_force_parity import ROOT, definition


SOURCE = ROOT / "GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/SlowDeathBehavior.cpp"
TRACE = ROOT / (
    "GeneralsRust/Code/Main/src/game_logic/world_objects/create_destroy_die/"
    "death/slow_death_same_frame.trace"
)

# Let the original methods choose when to draw, execute each OCL and remove the
# owner. The probe supplies installed definitions and observes the callbacks.
PROBES = r"""
#include <cassert>
#include <cstdio>
#include <vector>
#define DEBUG_ASSERTCRASH(condition, message) assert(condition)
#define MAKE_MODELCONDITION_MASK(flag) 0
using Real = float;
using Int = int;
using UnsignedInt = unsigned;
using ObjectID = unsigned;
using UpdateSleepTime = int;
enum { UPDATE_SLEEP_NONE, KINDOF_SHRUBBERY, DISABLED_HELD };
enum { FLUNG_INTO_AIR, BOUNCED, MIDPOINT_EXECUTED };
enum SlowDeathPhaseType { SDPHASE_INITIAL, SDPHASE_MIDPOINT, SDPHASE_FINAL };
struct Coord3D { float z = 0; };
struct PhysicsBehavior { ObjectID getLastCollidee() { return 0; } };
struct Object {
    Coord3D position;
    const Coord3D* getPosition() { return &position; }
    void setPosition(const Coord3D*) {}
    void setPositionZ(float) {}
    bool isAboveTerrain() { return false; }
    bool isKindOf(int) { return false; }
    PhysicsBehavior* getPhysics() { return nullptr; }
    void setDisabled(int) {}
    void clearAndSetModelConditionFlags(int, int) {}
    void clearModelConditionFlags(int) {}
    void setModelConditionFlags(int) {}
};
struct {
    unsigned getFrame() { return 41; }
    Object* findObjectByID(ObjectID) { return nullptr; }
    void destroyObject(Object*) { std::puts("retire"); }
} logic, *TheGameLogic = &logic;
struct { float getSlowDeathScale() { return 1.0f; } } lod, *TheGameLODManager = &lod;
struct FXList { static void doFXObj(const FXList*, Object*, Object*) {} };
struct ObjectCreationList {
    const char* name;
    static void create(const ObjectCreationList* ocl, Object*, const Coord3D*) {
        if (ocl) std::printf("ocl %s\n", ocl->name);
    }
};
struct WeaponTemplate {};
struct {
    void createAndFireTempWeapon(const WeaponTemplate*, Object*, const Coord3D*) {}
} weapons, *TheWeaponStore = &weapons;
using FXListVec = std::vector<const FXList*>;
using OCLVec = std::vector<const ObjectCreationList*>;
using WeaponTemplateVec = std::vector<const WeaponTemplate*>;
struct SlowDeathBehaviorModuleData {
    FXListVec m_fx[3];
    OCLVec m_ocls[3];
    WeaponTemplateVec m_weapons[3];
    unsigned m_maskOfLoadedEffects = 1;
    float m_sinkRate = 0;
    bool hasNonLodEffects() const { return true; }
};
struct SlowDeathBehavior {
    SlowDeathBehaviorModuleData data;
    Object object;
    unsigned m_flags = 0, m_sinkFrame = 41, m_midpointFrame = 41, m_destructionFrame = 41;
    float m_acceleratedTimeScale = 1.0f;
    bool isSlowDeathActivated() const { return true; }
    Object* getObject() { return &object; }
    const SlowDeathBehaviorModuleData* getSlowDeathBehaviorModuleData() { return &data; }
    void doPhaseStuff(SlowDeathPhaseType);
    UpdateSleepTime update();
};
int GameLogicRandomValue(int lo, int hi) {
    assert(lo == 0 && hi == 0);
    std::puts("draw");
    return 0;
}
"""

MAIN = r"""
int main() {
    SlowDeathBehavior death;
    ObjectCreationList midpoint{"WreckToss"}, final{"FinalToss"};
    death.data.m_ocls[SDPHASE_MIDPOINT].push_back(&midpoint);
    death.data.m_ocls[SDPHASE_FINAL].push_back(&final);
    death.update();
}
"""


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cxx", default="c++")
    args = parser.parse_args()
    original = SOURCE.read_text()
    callbacks = [
        definition(original, r"void SlowDeathBehavior::doPhaseStuff\("),
        definition(original, r"UpdateSleepTime SlowDeathBehavior::update\("),
    ]
    with tempfile.TemporaryDirectory(prefix="slow-death-parity-") as directory:
        cpp = Path(directory) / "original.cpp"
        exe = Path(directory) / "original"
        cpp.write_text("\n".join([PROBES, *callbacks, MAIN]))
        subprocess.run([args.cxx, "-std=c++17", cpp, "-o", exe], check=True)
        observed = subprocess.check_output([exe], text=True)
    expected = TRACE.read_text()
    if observed != expected:
        raise RuntimeError(f"Original C++ trace differs:\n{observed}Expected:\n{expected}")
    print("Original C++ matched the Rust same-frame SlowDeath regression trace")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
