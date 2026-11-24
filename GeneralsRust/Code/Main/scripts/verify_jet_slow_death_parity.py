#!/usr/bin/env python3
"""Compare the original JetSlowDeath derived callbacks with the Rust trace."""

import argparse
from pathlib import Path
import subprocess
import tempfile

from verify_ocl_force_parity import ROOT, definition
from verify_slow_death_parity import FIXTURES


SOURCE = ROOT / "GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/JetSlowDeathBehavior.cpp"

# The base callback marks its place in the trace. The ordinary SlowDeath probe
# independently exercises the original base method bodies.
PROBE = r"""
#include <cassert>
#include <cstdio>
#include <string>
#define DEBUG_ASSERTCRASH(condition, message) assert(condition)
#define MAKE_OBJECT_STATUS_MASK(flag) flag
using Real = float;
using Bool = bool;
using UpdateSleepTime = int;
using ObjectID = unsigned;
constexpr bool TRUE = true, FALSE = false;
enum { OBJECT_STATUS_DECK_HEIGHT_OFFSET = 1, KINDOF_SHRUBBERY = 2, UPDATE_SLEEP_NONE = 0 };
enum PathfindLayerEnum { LAYER_GROUND = 1, LAYER_BRIDGE = 2 };
struct Coord3D { float x = 0, y = 0, z = 25; };
struct DamageInfo {};
struct PhysicsBehavior {
    float roll = 0, pitch = 0;
    void setRollRate(float value) { roll = value; }
    void setPitchRate(float value) { pitch = value; }
    ObjectID getLastCollidee() { return 0; }
};
struct Locomotor {
    float lift = 0, turn = 1;
    void setMaxLift(float value) { lift = value; }
    void setMaxTurnRate(float value) { turn = value; }
};
struct AI {
    Locomotor loco;
    Locomotor* getCurLocomotor() { return &loco; }
};
struct Object {
    Coord3D position;
    PhysicsBehavior physics;
    AI ai;
    bool retired = false, deck = false;
    float getHeightAboveTerrain() { return position.z; }
    bool isSignificantlyAboveTerrain() { return position.z > 9; }
    Object& getStatusBits() { return *this; }
    bool test(unsigned) { return deck; }
    void clearStatus(unsigned) { deck = false; }
    const Coord3D* getPosition() { return &position; }
    void setLayer(PathfindLayerEnum) {}
    PhysicsBehavior* getPhysics() { return &physics; }
    AI* getAIUpdateInterface() { return &ai; }
    ObjectID getID() { return 1; }
    bool isKindOf(unsigned) { return false; }
};
struct GameLogic {
    unsigned frame = 41;
    unsigned getFrame() { return frame; }
    void destroyObject(Object* object) { object->retired = true; }
    Object* findObjectByID(ObjectID) { return nullptr; }
} game;
auto* TheGameLogic = &game;
struct Terrain {
    PathfindLayerEnum getLayerForDestination(const Coord3D*) { return LAYER_GROUND; }
    float getLayerHeight(float, float, PathfindLayerEnum) { return 0; }
} terrain;
auto* TheTerrainLogic = &terrain;
struct GlobalData { float m_gravity = -1; } global;
auto* TheGlobalData = &global;

struct EventName : std::string { bool isEmpty() const { return empty(); } };
struct AudioEvent {
    EventName getEventName() { return {}; }
    void setObjectID(ObjectID) {}
    void setPlayingHandle(int) {}
    int getPlayingHandle() { return 0; }
};
struct Audio {
    int addAudioEvent(AudioEvent*) { return 0; }
    void removeAudioEvent(int) {}
} audio;
auto* TheAudio = &audio;
struct FXList { static void doFXObj(const FXList*, Object*) {} };
struct ObjectCreationList {
    const char* name;
    static void create(const ObjectCreationList* list, Object*, void*) {
        if (list) std::printf("%u %s\n", game.frame, list->name);
    }
};
struct SlowDeathBehavior {
    Object object;
    bool active = false;
    Object* getObject() { return &object; }
    bool isSlowDeathActivated() const { return active; }
    virtual void beginSlowDeath(const DamageInfo*) {
        active = true;
        std::printf("%u BaseInitial\n", game.frame);
    }
    void onDie(const DamageInfo* damage) { beginSlowDeath(damage); }
    void update() {
        if (game.frame == 45) {
            std::printf("%u BaseFinal\n", game.frame);
            game.destroyObject(&object);
        }
    }
};
struct JetSlowDeathBehaviorModuleData {
    const FXList *m_fxOnGroundDeath{}, *m_fxInitialDeath{}, *m_fxHitGround{}, *m_fxSecondary{}, *m_fxFinalBlowUp{};
    const ObjectCreationList *m_oclOnGroundDeath{}, *m_oclInitialDeath{}, *m_oclHitGround{}, *m_oclSecondary{}, *m_oclFinalBlowUp{};
    AudioEvent m_deathLoopSound;
    float m_rollRate = 0.125f, m_rollRateDelta = 0.5f, m_fallHowFast = 1.1f, m_pitchRate = 0.375f;
    unsigned m_delaySecondaryFromInitialDeath = 3, m_delayFinalBlowUpFromHitGround = 0;
};
struct JetSlowDeathBehavior : SlowDeathBehavior {
    JetSlowDeathBehaviorModuleData data;
    const JetSlowDeathBehaviorModuleData* getJetSlowDeathBehaviorModuleData() { return &data; }
    unsigned m_timerDeathFrame = 0, m_timerOnGroundFrame = 0;
    float m_rollRate = 0;
    AudioEvent m_deathLoopSound;
    void onDie(const DamageInfo*);
    void beginSlowDeath(const DamageInfo*) override;
    UpdateSleepTime update();
};
"""
MAIN = r"""
int main() {
    ObjectCreationList initial{"JetInitial"}, contact{"JetContact"}, secondary{"JetSecondary"}, final{"JetFinal"};
    JetSlowDeathBehavior jet;
    jet.data.m_oclInitialDeath = &initial;
    jet.data.m_oclHitGround = &contact;
    jet.data.m_oclSecondary = &secondary;
    jet.data.m_oclFinalBlowUp = &final;
    jet.onDie(nullptr);
    assert(jet.m_timerDeathFrame == 41 && jet.object.ai.loco.turn == 0);
    assert(jet.object.ai.loco.lift == 1.0f - jet.data.m_fallHowFast);

    for (game.frame = 42; game.frame <= 45; ++game.frame) {
        if (game.frame == 44) jet.object.position.z = 0;
        jet.update();
        if (game.frame == 43) assert(jet.object.physics.roll == 0.0625f);
        if (game.frame == 44) {
            assert(!jet.object.retired && jet.m_timerOnGroundFrame == 44);
            assert(jet.m_timerDeathFrame == 0 && jet.object.physics.pitch == 0.375f);
        }
    }
    assert(jet.object.retired);
}
"""


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cxx", default="c++")
    args = parser.parse_args()
    original = SOURCE.read_text()
    callbacks = [
        definition(original, r"void JetSlowDeathBehavior::onDie\("),
        definition(original, r"void JetSlowDeathBehavior::beginSlowDeath\("),
        definition(original, r"UpdateSleepTime JetSlowDeathBehavior::update\("),
    ]
    with tempfile.TemporaryDirectory(prefix="jet-slow-death-parity-") as directory:
        cpp = Path(directory) / "original.cpp"
        exe = Path(directory) / "original"
        cpp.write_text("\n".join([PROBE, *callbacks, MAIN]))
        subprocess.run([args.cxx, "-std=c++17", cpp, "-o", exe], check=True)
        observed = subprocess.check_output([exe], text=True)
    expected = (FIXTURES / "jet_slow_death_order.trace").read_text()
    if observed != expected:
        raise RuntimeError(f"Original C++ JetSlowDeath trace differs:\n{observed}Expected:\n{expected}")
    print("Original C++ matched the Rust JetSlowDeath callback trace")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
