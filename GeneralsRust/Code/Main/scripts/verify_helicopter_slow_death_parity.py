"""Verify the ordered rotorcraft death trace using the original C++ callbacks."""
from pathlib import Path
import argparse
import subprocess
import tempfile

from verify_ocl_force_parity import ROOT, definition
from verify_slow_death_parity import FIXTURES

SOURCE = ROOT / "GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/HelicopterSlowDeathUpdate.cpp"
MATRIX = ROOT / "GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/matrix3d.h"

STUBS = r"""
#include <cassert>
#include <cmath>
#include <cstdio>
using Bool = bool;
using Real = float;
using ObjectID = unsigned;
using UpdateSleepTime = int;
constexpr bool TRUE = true, FALSE = false;
constexpr int DISABLED_HELD = 1, MODELCONDITION_SPECIAL_DAMAGED = 2;
constexpr int KINDOF_SHRUBBERY = 1, UPDATE_SLEEP_NONE = 0, LEVEL_REGULAR = 0;
constexpr int LOGICFRAMES_PER_SECOND = 30;
constexpr Real INVALID_ANGLE = -999.0f;
#define DEBUG_ASSERTCRASH(condition, message) assert(condition)
#define DOUBLE_TO_REAL(x) static_cast<float>(x)
#define Cos(x) std::cos(x)
#define Sin(x) std::sin(x)
#define ORBIT_DIRECTION_LEFT (1.0f)
enum PathfindLayerEnum { LAYER_BRIDGE = 3 };
struct Coord3D { Real x = 0, y = 0, z = 0; };
struct Matrix3D {
    Real Row[3][4]{{1, 0, 0, 10}, {0, 1, 0, 20}, {0, 0, 1, 25}};
    void In_Place_Pre_Rotate_Z(Real angle);
    Real rotation() const { return std::atan2(Row[1][0], Row[0][0]); }
    Coord3D transform(const Coord3D& p) const {
        const auto axis = [&](int i) { return Row[i][0]*p.x + Row[i][1]*p.y + Row[i][2]*p.z + Row[i][3]; };
        return {axis(0), axis(1), axis(2)};
    }
};
#define WWINLINE inline
struct DamageInfo {};
unsigned frame = 41;
void trace(const char* label) { std::printf("%u %s\n", frame, label); }
void trace_position(const char* label, const Coord3D& p) {
    std::printf("%u %s %.3f %.3f %.3f\n", frame, label, p.x, p.y, p.z);
}
struct Name {
    const char* value = "";
    bool isEmpty() const { return *value == '\0'; }
    const char* str() const { return value; }
};
struct Drawable {
    const Matrix3D* matrix;
    void stopAmbientSound() { trace("ambient_stop"); }
    bool getPristineBonePositions(const char* name, int, Coord3D* out, void*, int) {
        if (*name == 'B') *out = {2, 3, 4};
        else *out = {1, 0, 0};
        return true;
    }
    void convertBonePosToWorldPos(const Coord3D* in, void*, Coord3D* out, void*) {
        *out = matrix->transform(*in);
    }
    void setModelConditionState(int state) {
        assert(state == MODELCONDITION_SPECIAL_DAMAGED);
        trace("special_damaged");
    }
};
struct PhysicsBehavior {
    void applyMotiveForce(const Coord3D* force) { trace_position("motive", *force); }
    ObjectID getLastCollidee() { return 0; }
};
struct Locomotor {
    void setMaxLift(Real value) { std::printf("%u lift %.3f\n", frame, value); }
    void setMaxBraking(Real value) { std::printf("%u brake %.3f\n", frame, value); }
};
struct AI {
    Locomotor loco;
    Locomotor* getCurLocomotor() { return &loco; }
};
struct Object {
    Coord3D position{10, 20, 25};
    Matrix3D matrix;
    PhysicsBehavior physics;
    Drawable drawable{&matrix};
    AI ai;
    unsigned getID() { return 7; }
    const Coord3D* getPosition() { return &position; }
    const Matrix3D* getTransformMatrix() { return &matrix; }
    void setTransformMatrix(const Matrix3D* value) {
        matrix = *value;
        std::printf("%u rotation %.3f\n", frame, matrix.rotation());
    }
    Real getOrientation() { return 0; }
    Drawable* getDrawable() { return &drawable; }
    AI* getAIUpdateInterface() { return &ai; }
    PhysicsBehavior* getPhysics() { return &physics; }
    int getVeterancyLevel() { return 1; }
    int getTeam() { return 5; }
    bool isKindOf(int) { return false; }
    void setLayer(PathfindLayerEnum layer) {
        assert(layer == LAYER_BRIDGE);
        trace("bridge");
    }
    void setDisabled(int flag) {
        assert(flag == DISABLED_HELD);
        trace("held");
    }
};
struct GameLogic {
    unsigned getFrame() { return frame; }
    void destroyObject(Object*) { trace("retire"); }
    Object* findObjectByID(ObjectID) { return nullptr; }
} game;
auto* TheGameLogic = &game;
struct Terrain {
    PathfindLayerEnum getHighestLayerForDestination(const Coord3D* p) {
        assert(p->z == 99999.0f);
        return LAYER_BRIDGE;
    }
    Real getLayerHeight(Real, Real, PathfindLayerEnum) { return 8; }
} terrain;
auto* TheTerrainLogic = &terrain;
struct GlobalData { Real m_gravity = -1; } global;
auto* TheGlobalData = &global;
struct Event {
    Name name{ "Loop" };
    int handle = 0;
    Name getEventName() { return name; }
    void setObjectID(ObjectID id) { assert(id == 7); }
    void setPlayingHandle(int value) { handle = value; }
    int getPlayingHandle() { return handle; }
};
struct Audio {
    int addAudioEvent(Event*) { trace("loop_start"); return 42; }
    void removeAudioEvent(int handle) { assert(handle == 42); trace("loop_stop"); }
} audio;
auto* TheAudio = &audio;
struct FXList {
    const char* name;
    static void doFXObj(const FXList* fx, Object*) { trace(fx->name); }
    static void doFXPos(const FXList* fx, const Coord3D* p) { trace_position(fx->name, *p); }
};
struct ObjectCreationList {
    const char* name;
    static void create(const ObjectCreationList* list, Object*, void*) { trace(list->name); }
    static void create(const ObjectCreationList* list, Object* object, const Coord3D* p, void*, Real orientation) {
        assert(object->getID() == 7 && orientation == INVALID_ANGLE);
        trace_position(list->name, *p);
    }
};
struct EjectPilotDie {
    static void ejectPilot(const ObjectCreationList* list, Object*, void*) { trace(list->name); }
};
struct ParticleSystem {
    void setPosition(const Coord3D* p) { trace_position("particle_pos", *p); }
    void attachToObject(Object*) { trace("particle_attach"); }
};
struct ParticleSystemManager {
    ParticleSystem particle;
    ParticleSystem* createParticleSystem(const char*) { trace("particle_create"); return &particle; }
} particles;
auto* TheParticleSystemManager = &particles;
struct ThingTemplate {};
struct ThingFactory {
    ThingTemplate template_;
    Object rubble;
    const ThingTemplate* findTemplate(Name n) { assert(*n.str() == 'R'); return &template_; }
    Object* newObject(const ThingTemplate*, int team) {
        assert(team == 5);
        trace("HeliRubble");
        return &rubble;
    }
} factory;
auto* TheThingFactory = &factory;
Real GameLogicRandomValueReal(Real min, Real max) {
    assert(min == 2.73f && max == 2.73f);
    trace("blade_draw");
    return min;
}
struct SlowDeathBehavior {
    Object object;
    bool active = false;
    Object* getObject() { return &object; }
    bool isSlowDeathActivated() const { return active; }
    void beginSlowDeath(const DamageInfo*) { active = true; trace("HeliBaseInitial"); }
    void update() {
        trace("base_update");
        if (frame == 45) trace("HeliBaseFinal");
    }
};
struct HelicopterSlowDeathBehaviorModuleData {
    const char* m_attachParticleSystem = "Smoke";
    Name m_attachParticleBone{"Exhaust"}, m_bladeBone{"Blade"}, m_finalRubbleObject{"Rubble"};
    const Coord3D m_attachParticleLoc{};
    Event m_deathSound;
    const FXList *m_fxBlade, *m_fxHitGround, *m_fxFinalBlowUp;
    const ObjectCreationList *m_oclBlade, *m_oclEjectPilot, *m_oclHitGround, *m_oclFinalBlowUp;
    Real m_minBladeFlyOffDelay = 2.73f, m_maxBladeFlyOffDelay = 2.73f;
    Real m_spiralOrbitForwardSpeed = 2, m_spiralOrbitTurnRate = 1.57079632679f;
    Real m_spiralOrbitForwardSpeedDamping = 0.5f, m_minSelfSpin = 0.25f, m_maxSelfSpin = 0.75f;
    Real m_selfSpinUpdateDelay = 1.5f, m_selfSpinUpdateAmount = 30.0f;
    Real m_fallHowFast = 1.25f, m_maxBraking = 3, m_delayFromGroundToFinalDeath = 1.5f;
};
struct HelicopterSlowDeathBehavior : SlowDeathBehavior {
    HelicopterSlowDeathBehaviorModuleData data;
    int m_orbitDirection = 1;
    Real m_forwardAngle = 0, m_forwardSpeed = 0, m_selfSpin = 0;
    Bool m_selfSpinTowardsMax = false;
    unsigned m_lastSelfSpinUpdateFrame = 0, m_bladeFlyOffFrame = 0, m_hitGroundFrame = 0;
    Event m_deathSound;
    const HelicopterSlowDeathBehaviorModuleData* getHelicopterSlowDeathBehaviorModuleData() { return &data; }
    void beginSlowDeath(const DamageInfo*);
    UpdateSleepTime update();
};
"""
MAIN = r"""
int main() {
    FXList blade_fx{"blade_fx"}, contact_fx{"contact_fx"}, final_fx{"final_fx"};
    ObjectCreationList blade{"HeliBlade"}, eject{"HeliPilot"}, contact{"HeliContact"}, final{"HeliFinal"};
    HelicopterSlowDeathBehavior heli;
    heli.data.m_fxBlade = &blade_fx;
    heli.data.m_fxHitGround = &contact_fx;
    heli.data.m_fxFinalBlowUp = &final_fx;
    heli.data.m_oclBlade = &blade;
    heli.data.m_oclEjectPilot = &eject;
    heli.data.m_oclHitGround = &contact;
    heli.data.m_oclFinalBlowUp = &final;
    heli.beginSlowDeath(nullptr);
    assert(heli.m_bladeFlyOffFrame == 2);
    for (frame = 42; frame <= 45; ++frame) {
        if (frame == 43) {
            heli.object.position.z = 8.5f;
            heli.object.matrix.Row[2][3] = 8.5f;
        }
        heli.update();
    }
}
"""

def main() -> None:
    args = argparse.ArgumentParser(description=__doc__)
    args.add_argument("--cxx", default="c++")
    args.add_argument("--print-trace", action="store_true")
    options = args.parse_args()
    source = SOURCE.read_text()
    callbacks = [
        definition(MATRIX.read_text(), r"WWINLINE void Matrix3D::In_Place_Pre_Rotate_Z\(float theta\)"),
        definition(source, r"void HelicopterSlowDeathBehavior::beginSlowDeath\("),
        definition(source, r"UpdateSleepTime HelicopterSlowDeathBehavior::update\("),
    ]
    with tempfile.TemporaryDirectory(prefix="heli-death-original-") as directory:
        root = Path(directory)
        cpp, exe = root / "probe.cpp", root / "probe"
        cpp.write_text("\n".join([STUBS, *callbacks, MAIN]))
        subprocess.run([options.cxx, "-std=c++17", cpp, "-o", exe], check=True)
        observed = subprocess.check_output([exe], text=True)
    if options.print_trace:
        print(observed, end="")
        return
    expected = (FIXTURES / "helicopter_slow_death.trace").read_text()
    if observed != expected:
        raise RuntimeError(f"Original helicopter crash trace differs:\n{observed}Expected:\n{expected}")
    print("Original C++ matched the rotorcraft callback trace")

if __name__ == "__main__":
    main()
