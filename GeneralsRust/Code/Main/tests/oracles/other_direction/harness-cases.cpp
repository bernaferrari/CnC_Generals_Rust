std::uint32_t bits(Real value) {
    std::uint32_t b; std::memcpy(&b,&value,sizeof(b)); return b;
}
void hex(Real value) { std::printf(" %08x",bits(value)); }
void vec(Coord3D value) { hex(value.x); hex(value.y); hex(value.z); }
struct Case {
    const char* name;
    Real entry_angle, supplied_post_angle;
    Coord3D target_offset; // Original XYZ; host Z has the opposite Y sign.
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
        // Raw original input conversions (distance/frame and distance/frame²).
        desired=ConvertVelocityInSecsToFrames(36.0f);
        loco.max_acceleration=ConvertAccelerationInSecsToFrames(90.0f);
        loco.braking=ConvertAccelerationInSecsToFrames(90.0f);
        loco.owned_template.m_ultraAccurateSlideIntoPlaceFactor=c.slide_factor_frames;
    } else {
        // Explicit host adapter: parsed/rebound distance/sec, and acceleration
        // times host fixed dt as an impulse. This is not original raw physics.
        desired=host_speed_binding(36.0f);
        loco.max_acceleration=host_accel_binding(90.0f)*SECONDS_PER_LOGICFRAME_REAL;
        loco.braking=host_accel_binding(90.0f)*SECONDS_PER_LOGICFRAME_REAL;
        // Keep the slide's distance threshold in the same world-space units
        // when desired speed is supplied in distance/sec instead of /frame.
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
    // Diagnostic host integration projection, never called an original engine
    // position update. The force test must observe one actual host march later.
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
    // Every input velocity is exactly zero, and every desired speed is positive.
    // Post angles/directions are separately supplied to the rotation stub.
    const Case cases[] = {
        {"axis_nonzero",0,0,{300,0,0}},
        {"turn_90",0,-PI/2,{0,-300,0}},
        {"turn_30",0,-PI/6,{300,-173.20508f,0}},
        {"reflect_90",0,PI/2,{0,300,0}},
        {"reflect_30",0,PI/6,{300,173.20508f,0}},
        {"entry_reflect_90",PI/2,0,{300,0,0}},
        {"slide_close",0,0,{10,-10,0},true,30},
        {"slide_close_flag_off",0,-PI/4,{10,-10,0},false,30},
        {"slide_far",0,-PI/2,{0,-300,0},true,30},
    };
    for(const auto& c:cases) { run(c,0); run(c,1); }
}
