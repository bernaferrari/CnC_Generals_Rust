use glam::Vec3;

/// ParticleSys.cpp:1351-1367: draw x/y/z in that order, reject the zero
/// vector, then normalize. This is the original cube-normalized distribution,
/// not a uniform sphere surface distribution or a unit-ball rejection sampler.
pub fn point_on_unit_sphere(mut draw: impl FnMut(f32, f32) -> f32) -> Vec3 {
    loop {
        let x = draw(-1.0, 1.0);
        let y = draw(-1.0, 1.0);
        let z = draw(-1.0, 1.0);
        if x != 0.0 || y != 0.0 || z != 0.0 {
            return Vec3::new(x, y, z).normalize();
        }
    }
}

/// ParticleSys.cpp:1489-1506: same order/rejection as the sphere, with z
/// sampled in [0,1]. Random state remains owned by the caller.
pub fn point_on_unit_hemisphere(mut draw: impl FnMut(f32, f32) -> f32) -> Vec3 {
    loop {
        let x = draw(-1.0, 1.0);
        let y = draw(-1.0, 1.0);
        let z = draw(0.0, 1.0);
        if x != 0.0 || y != 0.0 || z != 0.0 {
            return Vec3::new(x, y, z).normalize();
        }
    }
}
