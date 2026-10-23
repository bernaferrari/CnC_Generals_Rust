// Component harness: compiled original method, bounded non-engine receiver stubs.
#include <cstdint>
#include <cstring>
#include <functional>
#include <iomanip>
#include <iostream>
#include <limits>
#include <string>
using Real = float;
enum BodyDamageType { BODY_PRISTINE, BODY_DAMAGED, BODY_REALLYDAMAGED, BODY_RUBBLE };
struct GlobalDataStub { Real m_unitDamagedThresh=0.7f; Real m_unitReallyDamagedThresh=0.35f; };
static GlobalDataStub data;
static GlobalDataStub* TheGlobalData=&data;
#include "original_calc_damage_state.inc"
constexpr unsigned OBJECT_STATUS_UNDER_CONSTRUCTION=1;
struct StatusBitsStub {
    bool underConstruction=false;
    bool test(unsigned flag) const { return flag==OBJECT_STATUS_UNDER_CONSTRUCTION && underConstruction; }
};
struct ObjectStub {
    StatusBitsStub status;
    bool effectivelyDead=false;
    std::function<void(bool)> beforeDeadWrite;
    const StatusBitsStub& getStatusBits() const { return status; }
    void setEffectivelyDead(bool value) { if(beforeDeadWrite) beforeDeadWrite(value); effectivelyDead=value; }
};
static std::uint32_t bits(Real value) { std::uint32_t out; std::memcpy(&out,&value,sizeof out); return out; }
class ActiveBody {
public:
    Real m_prevHealth=100, m_currentHealth=100, m_maxHealth=100;
    BodyDamageType m_curDamageState=BODY_PRISTINE;
    ObjectStub object;
    std::string scenario;
    unsigned visualCalls=0;
    ActiveBody() { object.beforeDeadWrite=[this](bool next){ trace("before_dead_write",next); }; }
    ObjectStub* getObject() { return &object; }
    // Bounded setCorrectDamageState stub: exact original calcDamageState above;
    // no structure/rubble geometry, pathfinder or particles are represented.
    void setCorrectDamageState() { m_curDamageState=calcDamageState(m_currentHealth,m_maxHealth); trace("classified",object.effectivelyDead); }
    // Observation stub, not an implementation of Drawable/audio/particle effects.
    void evaluateVisualCondition() { ++visualCalls; trace("visual",object.effectivelyDead); }
    void internalChangeHealth(Real delta);
    void trace(const char* phase,bool requestedDead) const {
        std::cout << "{\"scenario\":\"" << scenario << "\",\"phase\":\"" << phase
            << "\",\"hp\":" << std::setprecision(std::numeric_limits<Real>::max_digits10) << m_currentHealth
            << ",\"hp_bits\":" << bits(m_currentHealth) << ",\"previous_bits\":" << bits(m_prevHealth)
            << ",\"state\":" << static_cast<int>(m_curDamageState)
            << ",\"effectively_dead\":" << (object.effectivelyDead ? "true":"false")
            << ",\"requested_dead\":" << (requestedDead ? "true":"false")
            << ",\"under_construction\":" << (object.status.underConstruction ? "true":"false")
            << ",\"visual_calls\":" << visualCalls << "}\n";
    }
    void run(const char* name,Real delta) { scenario=name; internalChangeHealth(delta);trace("complete",object.effectivelyDead); }
};
#include "original_internal_change_health.inc"
int main() {
    ActiveBody body;
    body.run("unchanged_capped",10.0f);
    body.run("damage_crossing",-40.0f);
    body.run("unchanged_damage_state",-1.0f);
    body.run("lethal_crossing",-100.0f);
    body.run("heal_crossing_from_dead",50.0f);
    body.object.status.underConstruction=true;
    body.run("construction_crossing",-30.0f);
    body.object.status.underConstruction=false;
    body.run("heal_to_pristine",80.0f);
    body.object.effectivelyDead=true; // Original AI may reassert this flag.
    body.run("unchanged_health_clears_ai_dead",0.0f);
    ActiveBody thresholds;
    thresholds.run("damaged_threshold_equal",-30.0f);
    thresholds.run("really_damaged_threshold_equal",-35.0f);
    ActiveBody construction;
    construction.object.status.underConstruction=true;
    construction.run("construction_lethal",-100.0f);
    construction.object.status.underConstruction=false;
    construction.run("post_construction_same_state",0.0f);
    construction.run("post_construction_heal",100.0f);
    return 0;
}
