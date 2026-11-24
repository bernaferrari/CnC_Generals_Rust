#!/usr/bin/env python3
"""Replay the original SlowDeath selection and update against Rust test fixtures."""

import argparse
from pathlib import Path
import subprocess
import tempfile

from verify_ocl_force_parity import ROOT, definition


SOURCE = ROOT / "GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/SlowDeathBehavior.cpp"
FIXTURES = ROOT / (
    "GeneralsRust/Code/Main/src/game_logic/world_objects/create_destroy_die/"
    "death"
)

# Let the original methods choose when to draw, execute each OCL and remove the
# owner. The probe supplies installed definitions and observes the callbacks.
PROBES = r"""
#include <cassert>
#include <cstdio>
#include <algorithm>
#include <vector>
#define DEBUG_ASSERTCRASH(condition, message) assert(condition)
#define DEBUG_CRASH(message) assert(false)
#define MAKE_MODELCONDITION_MASK(flag) 0
using Real = float;
using Int = int;
using UnsignedInt = unsigned;
using ObjectID = unsigned;
using UpdateSleepTime = int;
using std::max;
constexpr unsigned PLAYERMASK_ALL = ~0u;
constexpr bool TRUE = true;
enum { UPDATE_SLEEP_NONE, KINDOF_SHRUBBERY, DISABLED_HELD };
enum { FLUNG_INTO_AIR, BOUNCED, MIDPOINT_EXECUTED };
enum SlowDeathPhaseType { SDPHASE_INITIAL, SDPHASE_MIDPOINT, SDPHASE_FINAL };
struct Coord3D { float z = 0; };
struct DamageInfo {
    struct { float m_actualDamageDealt, m_actualDamageClipped; } out;
};
struct Body { float getMaxHealth() { return 100.0f; } };
struct AIUpdateInterface {
    bool dead = false;
    bool isAiInDeadState() { return dead; }
    void markAsDead() { dead = true; }
};
struct SlowDeathBehaviorInterface {
    virtual bool isDieApplicable(const DamageInfo*) const = 0;
    virtual Int getProbabilityModifier(const DamageInfo*) const = 0;
    virtual void beginSlowDeath(const DamageInfo*) = 0;
};
struct BehaviorModule {
    SlowDeathBehaviorInterface* death;
    SlowDeathBehaviorInterface* getSlowDeathBehaviorInterface() { return death; }
};
struct PhysicsBehavior { ObjectID getLastCollidee() { return 0; } };
struct Object {
    Coord3D position;
    Body body;
    AIUpdateInterface ai;
    BehaviorModule** modules = nullptr;
    Body* getBodyModule() { return &body; }
    AIUpdateInterface* getAIUpdateInterface() { return &ai; }
    BehaviorModule** getBehaviorModules() { return modules; }
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
    void deselectObject(Object*, unsigned, bool) {}
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
    int m_probabilityModifier = 10;
    float m_modifierBonusPerOverkillPercent = 0;
    bool hasNonLodEffects() const { return true; }
};
struct SlowDeathBehavior : SlowDeathBehaviorInterface {
    SlowDeathBehaviorModuleData data;
    Object object;
    Object* owner = &object;
    const char* name = nullptr;
    unsigned m_flags = 0, m_sinkFrame = 41, m_midpointFrame = 41, m_destructionFrame = 41;
    float m_acceleratedTimeScale = 1.0f;
    bool isSlowDeathActivated() const { return true; }
    Object* getObject() const { return owner; }
    const SlowDeathBehaviorModuleData* getSlowDeathBehaviorModuleData() const { return &data; }
    bool isDieApplicable(const DamageInfo*) const override { return true; }
    Int getProbabilityModifier(const DamageInfo*) const override;
    void beginSlowDeath(const DamageInfo*) override { std::puts(name); }
    void onDie(const DamageInfo*);
    void doPhaseStuff(SlowDeathPhaseType);
    UpdateSleepTime update();
};
int GameLogicRandomValue(int lo, int hi) {
    if (lo == 1) {
        assert(hi > 0);
        std::printf("%d ", hi);
        return 100 % hi + lo;
    }
    assert(lo == 0 && hi == 0);
    std::puts("draw");
    return 0;
}
"""

MAIN = r"""
static void select(float dealt) {
    SlowDeathBehavior regular, overkill;
    regular.name = "Regular";
    overkill.name = "Overkill";
    overkill.data.m_modifierBonusPerOverkillPercent = 1000.0f;
    overkill.owner = regular.owner;
    BehaviorModule first{&regular}, second{&overkill};
    BehaviorModule* modules[]{&first, &second, nullptr};
    regular.owner->modules = modules;
    DamageInfo damage{{dealt, 100.0f}};
    std::printf("%.1f ", dealt);
    regular.onDie(&damage);
    overkill.onDie(&damage);
}

int main(int argc, char**) {
    if (argc > 1) {
        select(100.9f);
        select(102.5f);
        return 0;
    }
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
    on_die = definition(original, r"void SlowDeathBehavior::onDie\(")
    # MSVC6 let the first for-loop variable escape its scope. Redeclare the
    # iterator in the second loop for standard C++ without changing either loop.
    on_die = on_die.replace("/* UpdateModuleInterface** */ update =", "BehaviorModule** update =")
    callbacks = [
        definition(original, r"void SlowDeathBehavior::doPhaseStuff\("),
        definition(original, r"UpdateSleepTime SlowDeathBehavior::update\("),
        definition(original, r"Int SlowDeathBehavior::getProbabilityModifier\("),
        on_die,
    ]
    with tempfile.TemporaryDirectory(prefix="slow-death-parity-") as directory:
        cpp = Path(directory) / "original.cpp"
        exe = Path(directory) / "original"
        cpp.write_text("\n".join([PROBES, *callbacks, MAIN]))
        subprocess.run([args.cxx, "-std=c++17", cpp, "-o", exe], check=True)
        for fixture, extra in [
            ("slow_death_same_frame.trace", []),
            ("slow_death_selection.trace", ["select"]),
        ]:
            observed = subprocess.check_output([exe, *extra], text=True)
            expected = (FIXTURES / fixture).read_text()
            if observed != expected:
                raise RuntimeError(
                    f"Original C++ trace differs in {fixture}:\n{observed}Expected:\n{expected}"
                )
    print("Original C++ matched the Rust SlowDeath selection and same-frame traces")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
