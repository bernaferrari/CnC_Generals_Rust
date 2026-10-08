static_assert(sizeof(Real)==4 && std::numeric_limits<Real>::is_iec559);
using BodyDamageType = Int;
struct Body { BodyDamageType getDamageState() const { return 0; } };
struct Object {
    Coord3D position{100,-100,0}, direction{1,0,0}, supplied_post_direction{};
    Coord3D recorded_rotation_goal{};
    Real angle=0, supplied_post_angle=0;
    Int rotate_calls=0;
    Body body;
    const Coord3D* getPosition() const { return &position; }
    const Coord3D* getUnitDirectionVector2D() const { return &direction; }
    const Body* getBodyModule() const { return &body; }
};
struct GameLogicStub { UnsignedInt getFrame() const { return 100; } };
GameLogicStub logic_stub;
GameLogicStub* TheGameLogic=&logic_stub;
struct PhysicsBehavior {
    Object* object=nullptr;
    Coord3D m_vel{0,0,0}, m_accel{0,0,0};
    Real mass=1;
    UnsignedInt m_motiveForceExpires=110;
    mutable Int mass_reads=0;
    Int turning_calls=0;
    PhysicsTurningType turning=TURN_POSITIVE;
    const Object* getObject() const { return object; }
    Real getMass() const { ++mass_reads; return mass; }
    void setTurning(PhysicsTurningType value) { turning=value; ++turning_calls; }
    Real getForwardSpeed2D() const;
    Bool isMotive() const;
    void applyForce(const Coord3D* force);
    void applyMotiveForce(const Coord3D* force);
    void integrate_original_velocity();
};
struct LocomotorTemplate {
    Real m_minSpeed=0, m_ultraAccurateSlideIntoPlaceFactor=0;
};
struct Locomotor {
    LocomotorTemplate owned_template;
    const LocomotorTemplate* m_template=&owned_template;
    Real max_acceleration=0, max_speed=0, braking=0;
    Bool ultra_accurate=false;
    enum { ULTRA_ACCURATE, NO_SLOW_DOWN_AS_APPROACHING_DEST };
    Real getMaxAcceleration(BodyDamageType) const { return max_acceleration; }
    Real getMaxSpeedForCondition(BodyDamageType) const { return max_speed; }
    Real getBraking() const { return braking; }
    Bool getFlag(Int flag) const {
        // Approach remains enabled. Far nonzero-speed cases keep the path
        // distance beyond every selected scalar's slowdown distance. The
        // close-slide sentinel starts at rest, so its slowdown is zero.
        return flag==ULTRA_ACCURATE ? ultra_accurate : false;
    }
    PhysicsTurningType rotateTowardsPosition(Object* obj, const Coord3D& goal) {
        // SUPPLIED TURN OUTPUT, not original rotation execution. The force
        // expectation cannot establish turn rates, matrix/pivot, or trajectory.
        ++obj->rotate_calls;
        obj->recorded_rotation_goal=goal;
        obj->direction=obj->supplied_post_direction;
        obj->angle=obj->supplied_post_angle;
        return TURN_NONE; // A completed in-bound turn can return TURN_NONE.
    }
    void moveTowardsPositionOther(Object*,PhysicsBehavior*,const Coord3D&,Real,Real);
};
