static_assert(sizeof(Real)==4 && std::numeric_limits<Real>::is_iec559);
struct Coord3D { Real x=0,y=0,z=0; };
struct Body { Int getDamageState() const { return 0; } };
struct Object {
    Coord3D position, direction, supplied_post_direction;
    Real angle=0, supplied_post_angle=0, requested_angle=0;
    Int rotate_calls=0;
    Body body;
    const Coord3D* getPosition() const { return &position; }
    const Coord3D* getUnitDirectionVector2D() const { return &direction; }
    Real getOrientation() const { return angle; }
    const Body* getBodyModule() const { return &body; }
};
struct GameLogicStub { UnsignedInt getFrame() const { return 100; } };
GameLogicStub logic_stub;
GameLogicStub* TheGameLogic=&logic_stub;
struct PhysicsBehavior {
    Object* object=nullptr;
    Coord3D m_vel, m_accel;
    Real mass=1;
    UnsignedInt m_motiveForceExpires=110;
    mutable Int mass_reads=0;
    const Object* getObject() const { return object; }
    Real getMass() const { ++mass_reads; return mass; }
    Real getForwardSpeed2D() const;
    Bool isMotive() const;
    void applyForce(const Coord3D* force);
    void applyMotiveForce(const Coord3D* force);
    void integrate_original_velocity();
};
struct LocomotorTemplate { Real m_wanderWidthFactor=0, m_minSpeed=0; };
struct Locomotor {
    LocomotorTemplate owned_template;
    const LocomotorTemplate* m_template=&owned_template;
    Real max_acceleration=0, max_speed=0, braking=0;
    Real m_angleOffset=0, m_offsetIncrement=0;
    Bool increasing=true;
    enum { OFFSET_INCREASING, NO_SLOW_DOWN_AS_APPROACHING_DEST };
    Bool getIsDownhillOnly() const { return false; }
    Real getMaxAcceleration(Int) const { return max_acceleration; }
    Real getMaxSpeedForCondition(Int) const { return max_speed; }
    Real getBraking() const { return braking; }
    Bool getFlag(Int flag) const {
        // Approach is explicitly disabled in every oracle case. This keeps
        // host-impulse lane slowdown arithmetic from choosing a force target.
        return flag==NO_SLOW_DOWN_AS_APPROACHING_DEST ? true : increasing;
    }
    void setFlag(Int flag, Bool value) { if(flag==OFFSET_INCREASING) increasing=value; }
    void locoUpdate_moveTowardsAngle(Object* obj, Real requested) {
        // STUB: record original requested angle; install separately supplied
        // post-turn state. No original rotation/matrix/max-turn-rate executes.
        ++obj->rotate_calls;
        obj->requested_angle=requested;
        obj->direction=obj->supplied_post_direction;
        obj->angle=obj->supplied_post_angle;
    }
    void moveTowardsPositionLegs(Object*,PhysicsBehavior*,const Coord3D&,Real,Real);
};
