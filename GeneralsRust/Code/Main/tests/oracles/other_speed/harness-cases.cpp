std::uint32_t bits(Real value) {
    std::uint32_t b; std::memcpy(&b,&value,sizeof(b)); return b;
}
void hex(Real value) { std::printf(" %08x",bits(value)); }
void vec(Coord3D value) { hex(value.x); hex(value.y); hex(value.z); }
struct Case {
    const char* name;
    Coord3D velocity; // Explicit host input; converted for raw original lane.
    Real entry_angle, supplied_post_angle;
    Coord3D target_offset;
    Real speed=36, acceleration=90, braking=90;
    Bool ultra_accurate=false;
    Real slide_factor_frames=0;
};
Real host_speed_binding(Real value) {
    return ConvertVelocityInSecsToFrames(value)*LOGICFRAMES_PER_SECONDS_REAL;
}
Real host_accel_binding(Real value) {
    return ConvertAccelerationInSecsToFrames(value)*LOGICFRAMES_PER_SECONDS_REAL*LOGICFRAMES_PER_SECONDS_REAL;
}
void run(const Case& c, Int lane) {
    Object obj;
    obj.angle=c.entry_angle;
    obj.direction={cosf(c.entry_angle),sinf(c.entry_angle),0};
    obj.supplied_post_angle=c.supplied_post_angle;
    obj.supplied_post_direction={cosf(c.supplied_post_angle),sinf(c.supplied_post_angle),0};
    PhysicsBehavior physics;
    physics.object=&obj;
    Locomotor loco;
    loco.ultra_accurate=c.ultra_accurate;
    Real desired;
    if(lane==0) {
        physics.m_vel={ConvertVelocityInSecsToFrames(c.velocity.x),
            ConvertVelocityInSecsToFrames(c.velocity.y),ConvertVelocityInSecsToFrames(c.velocity.z)};
        desired=ConvertVelocityInSecsToFrames(c.speed);
        loco.max_acceleration=ConvertAccelerationInSecsToFrames(c.acceleration);
        loco.braking=ConvertAccelerationInSecsToFrames(c.braking);
        loco.owned_template.m_ultraAccurateSlideIntoPlaceFactor=c.slide_factor_frames;
    } else {
        // Existing host input velocity is not an authored template field.
        physics.m_vel=c.velocity;
        desired=host_speed_binding(c.speed);
        loco.max_acceleration=host_accel_binding(c.acceleration)*SECONDS_PER_LOGICFRAME_REAL;
        loco.braking=host_accel_binding(c.braking)*SECONDS_PER_LOGICFRAME_REAL;
        // Explicit threshold adapter for speed supplied as distance/sec.
        loco.owned_template.m_ultraAccurateSlideIntoPlaceFactor=
            c.slide_factor_frames/LOGICFRAMES_PER_SECONDS_REAL;
    }
    loco.max_speed=desired;
    const Coord3D entry_direction=obj.direction;
    const Coord3D before=physics.m_vel;
    const Coord3D target{obj.position.x+c.target_offset.x,
        obj.position.y+c.target_offset.y,obj.position.z+c.target_offset.z};
    const Real path_distance=c.target_offset.length();
    const Real entry_speed=physics.getForwardSpeed2D();
    loco.moveTowardsPositionOther(&obj,&physics,target,path_distance,desired);
    const Real exit_speed=physics.getForwardSpeed2D();
    const Coord3D impulse=physics.m_accel;
    physics.integrate_original_velocity();
    // Diagnostic projection only; original full physics/friction is absent.
    const Real scale=lane==0 ? 1.0f : SECONDS_PER_LOGICFRAME_REAL;
    const Coord3D projected_displacement{physics.m_vel.x*scale,
        physics.m_vel.y*scale,physics.m_vel.z*scale};
    std::printf("%s %s",lane==0?"raw-frame":"host-impulse",c.name);
    vec(before); vec(obj.position); vec(target);
    hex(c.entry_angle); hex(c.supplied_post_angle);
    vec(entry_direction); vec(obj.supplied_post_direction);
    hex(desired); hex(loco.max_acceleration); hex(loco.braking);
    hex(physics.mass); hex(entry_speed); hex(exit_speed);
    hex(c.slide_factor_frames); hex(loco.owned_template.m_ultraAccurateSlideIntoPlaceFactor);
    hex(path_distance); hex(obj.angle); vec(obj.direction);
    vec(impulse); vec(physics.m_vel); vec(projected_displacement);
    std::printf(" %d %d %d %d %d %u\n",Int(c.ultra_accurate),obj.rotate_calls,
        physics.turning_calls,Int(physics.turning),physics.mass_reads,physics.m_motiveForceExpires);
}
int main() {
    const Case cases[] = {
        {"axis_accel",{30,0,0},0,0,{1000,0,0}},
        {"metric_diagonal",{30,-30,0},-PI/4,-PI/4,{1000,-1000,0}},
        {"metric_reflection",{30,30,0},PI/4,PI/4,{1000,1000,0}},
        {"cancellation",{30,30,0},-PI/4,-PI/4,{1000,-1000,0},24},
        {"negative_diagonal",{-30,30,0},-PI/4,-PI/4,{1000,-1000,0},6,1800},
        {"negative_axis_control",{-30,0,0},0,0,{1000,0,0},6,1800},
        {"timing_turn30",{30,0,0},0,-PI/6,{1000,-577.350269f,0},27},
        {"timing_reflect30",{30,0,0},0,PI/6,{1000,577.350269f,0},27},
        {"timing_turn90",{30,0,0},0,-PI/2,{0,-1000,0},6},
        {"timing_negative_turn30",{-30,0,0},0,-PI/6,{1000,-577.350269f,0},6,1800},
        {"masking_diagonal_turn",{30,-30,0},-PI/4,0,{1000,0,0}},
        {"zero_turn_control",{0,0,0},0,-PI/2,{0,-1000,0}},
        {"slide_zero_control",{0,0,0},0,0,{10,-10,0},36,90,90,true,30},
    };
    for(const auto& c:cases) { run(c,0); run(c,1); }
}
