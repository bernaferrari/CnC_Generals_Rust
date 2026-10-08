std::uint32_t bits(Real value) {
    std::uint32_t b; std::memcpy(&b,&value,sizeof(b)); return b;
}
void hex(Real value) { std::printf(" %08x",bits(value)); }
void vec(Coord3D value) { hex(value.x); hex(value.y); hex(value.z); }
struct Case {
    const char* name;
    Coord3D velocity; // Original XYZ, supplied in distance/second for two lanes.
    Real initial_angle, goal_angle, post_angle;
    Real goal_speed, acceleration, braking;
    Real mass=1;
};
Real host_speed_binding(Real value) {
    return ConvertVelocityInSecsToFrames(value)*LOGICFRAMES_PER_SECONDS_REAL;
}
Real host_accel_binding(Real value) {
    return ConvertAccelerationInSecsToFrames(value)*LOGICFRAMES_PER_SECONDS_REAL*LOGICFRAMES_PER_SECONDS_REAL;
}
void run(const Case& c, Int lane) {
    // lane 0: raw frame conversion; lane 1: explicit host-impulse adapter;
    // lane 2: exact supplied frame scalars, with no seconds conversion.
    Object obj;
    obj.angle=c.initial_angle;
    obj.direction={cosf(c.initial_angle),sinf(c.initial_angle),0};
    obj.supplied_post_angle=c.post_angle;
    obj.supplied_post_direction={cosf(c.post_angle),sinf(c.post_angle),0};
    PhysicsBehavior physics;
    physics.object=&obj; physics.mass=c.mass;
    Locomotor loco;
    Real desired;
    if(lane==0) {
        physics.m_vel={ConvertVelocityInSecsToFrames(c.velocity.x),
            ConvertVelocityInSecsToFrames(c.velocity.y),ConvertVelocityInSecsToFrames(c.velocity.z)};
        desired=ConvertVelocityInSecsToFrames(c.goal_speed);
        loco.max_acceleration=ConvertAccelerationInSecsToFrames(c.acceleration);
        loco.braking=ConvertAccelerationInSecsToFrames(c.braking);
    } else if(lane==1) {
        // Host velocity is explicit prior runtime state; unlike bound template
        // scalars it is not parsed or round-tripped in this lane.
        physics.m_vel=c.velocity;
        desired=host_speed_binding(c.goal_speed);
        loco.max_acceleration=host_accel_binding(c.acceleration)*SECONDS_PER_LOGICFRAME_REAL;
        loco.braking=host_accel_binding(c.braking)*SECONDS_PER_LOGICFRAME_REAL;
    } else {
        physics.m_vel=c.velocity; desired=c.goal_speed;
        loco.max_acceleration=c.acceleration; loco.braking=c.braking;
    }
    loco.max_speed=desired;
    const Coord3D before=physics.m_vel;
    const Real initial_speed=physics.getForwardSpeed2D();
    const Coord3D entry_direction=obj.direction;
    // Supplied distant target for original atan2; no path/terrain executes.
    const Coord3D target{1000.0f*cosf(c.goal_angle),1000.0f*sinf(c.goal_angle),0};
    loco.moveTowardsPositionLegs(&obj,&physics,target,1000000.0f,desired);
    const Real speed_after_rotation=physics.getForwardSpeed2D();
    const Coord3D impulse=physics.m_accel;
    physics.integrate_original_velocity();
    const char* lane_name=lane==0?"raw-frame":lane==1?"host-impulse":"exact-frame";
    std::printf("%s %s",lane_name,c.name);
    vec(before); hex(c.initial_angle); hex(c.goal_angle); hex(c.post_angle);
    vec(entry_direction); vec(obj.supplied_post_direction);
    hex(desired); hex(loco.max_acceleration); hex(loco.braking); hex(c.mass);
    hex(initial_speed); hex(speed_after_rotation); hex(obj.requested_angle);
    vec(impulse); vec(physics.m_vel);
    std::printf(" %d %d %u\n",obj.rotate_calls,physics.mass_reads,physics.m_motiveForceExpires);
}
int main() {
    const Case live[] = {
        {"axis_accel",{30,0,0},0,0,0,36,90,90},
        {"diagonal_sign",{30,-30,0},-PI/4,-PI/4,-PI/4,36,90,90},
        {"diagonal_reflect",{30,30,0},PI/4,PI/4,PI/4,36,90,90},
        {"component_cancellation",{30,30,0},-PI/4,-PI/4,-PI/4,24,90,90},
        {"reverse_clamp",{-30,30,0},-PI/4,-PI/4,-PI/4,6,1800,90},
        {"turn_snapshot",{30,0,0},0,-PI/6,-PI/6,60,90,900},
        {"turn_limited",{30,0,0},0,-PI/6,-PI/12,60,90,900},
        {"vertical_control",{30,-30,90},-PI/4,-PI/4,-PI/4,36,90,90},
    };
    for(const auto& c:live) {run(c,0);run(c,1);}
    const Case scalar[] = {
        {"positive_brake_below",{3,0,0},0,0,0,2,1,0.5f},
        {"positive_brake_equal",{3,0,0},0,0,0,2,1,1},
        {"positive_brake_above",{3,0,0},0,0,0,2,1,2},
        {"negative_brake_below",{3,0,0},0,0,0,2,1,-0.5f},
        {"negative_brake_equal",{3,0,0},0,0,0,2,1,-1},
        {"negative_brake_above",{3,0,0},0,0,0,2,1,-2},
        {"zero_gap",{2,0,0},0,0,0,2,1,1},
        {"zero_braking",{3,0,0},0,0,0,2,1,0},
    };
    for(const auto& c:scalar) run(c,2);
}
