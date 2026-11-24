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
#define MAKE_MODELCONDITION_MASK(flag) flag
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
enum {
    MODELCONDITION_EXPLODED_FLAILING = 1,
    MODELCONDITION_EXPLODED_BOUNCING = 2,
    MODELCONDITION_PARACHUTING = 4
};
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
struct PhysicsBehavior {
    ObjectID last = 0;
    ObjectID getLastCollidee() { return last; }
};
struct Object {
    Coord3D position;
    Body body;
    AIUpdateInterface ai;
    BehaviorModule** modules = nullptr;
    int model = 0;
    bool shrub = false;
    bool held = false;
    bool retired = false;
    PhysicsBehavior physics;
    Body* getBodyModule() { return &body; }
    AIUpdateInterface* getAIUpdateInterface() { return &ai; }
    BehaviorModule** getBehaviorModules() { return modules; }
    const Coord3D* getPosition() { return &position; }
    void setPosition(const Coord3D* newPosition) { position = *newPosition; }
    void setPositionZ(float z) { position.z = z; }
    bool isAboveTerrain() { return position.z > 0.0f; }
    bool isKindOf(int kind) { return shrub && kind == KINDOF_SHRUBBERY; }
    PhysicsBehavior* getPhysics() { return &physics; }
    void setDisabled(int disable) { if (disable == DISABLED_HELD) held = true; }
    void clearAndSetModelConditionFlags(int clear, int set) { model = (model & ~clear) | set; }
    void clearModelConditionFlags(int flags) { model &= ~flags; }
    void setModelConditionFlags(int flags) { model |= flags; }
};
struct {
    unsigned frame = 41;
    Object* tree = nullptr;
    bool trace_retire = true;
    unsigned getFrame() { return frame; }
    Object* findObjectByID(ObjectID id) { return id == 1 ? tree : nullptr; }
    void deselectObject(Object*, unsigned, bool) {}
    void destroyObject(Object* owner) {
        owner->retired = true;
        if (trace_retire) std::puts("retire");
    }
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

static const char* flight_model(int flags) {
    switch (flags) {
        case MODELCONDITION_EXPLODED_FLAILING: return "flailing";
        case MODELCONDITION_EXPLODED_BOUNCING: return "bouncing";
        case MODELCONDITION_PARACHUTING: return "parachuting";
        default: return "unexpected";
    }
}

static void flight() {
    SlowDeathBehavior death;
    death.m_flags = 1 << FLUNG_INTO_AIR;
    death.m_sinkFrame = 56;
    death.m_midpointFrame = 58;
    death.m_destructionFrame = 71;
    death.object.model = MODELCONDITION_EXPLODED_FLAILING;
    for (unsigned frame = 42; frame <= 44; ++frame) {
        logic.frame = frame;
        death.object.position.z = frame == 42 ? 1.0f : 0.0f;
        death.update();
        std::printf("%u %u %u %s\n", death.m_sinkFrame - 56,
                    death.m_midpointFrame - 58, death.m_destructionFrame - 71,
                    flight_model(death.object.model));
    }
    SlowDeathBehavior contact;
    contact.m_flags = 1 << FLUNG_INTO_AIR;
    contact.m_sinkFrame = 56;
    contact.m_midpointFrame = 58;
    contact.m_destructionFrame = 71;
    contact.object.model = MODELCONDITION_EXPLODED_FLAILING;
    contact.object.physics.last = 1;
    contact.data.m_sinkRate = 1.0f;
    Object tree;
    tree.shrub = true;
    logic.tree = &tree;
    logic.trace_retire = false;
    contact.update();
    std::printf("%.0f %s %d %d\n", contact.object.position.z, flight_model(contact.object.model),
                int(contact.object.held), int(contact.object.retired));
}

int main(int argc, char** argv) {
    if (argc > 1 && argv[1][0] == 'f') {
        flight();
        return 0;
    }
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
            ("slow_death_flight.trace", ["flight"]),
        ]:
            observed = subprocess.check_output([exe, *extra], text=True)
            expected = (FIXTURES / fixture).read_text()
            if observed != expected:
                raise RuntimeError(
                    f"Original C++ trace differs in {fixture}:\n{observed}Expected:\n{expected}"
                )
    print("Original C++ matched the Rust SlowDeath selection, phase and flight traces")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
