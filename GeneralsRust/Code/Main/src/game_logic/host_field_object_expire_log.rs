//! Field lifetime categories passed directly to receiving Main operations.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldObjectKind {
    NukeRadiation,
    AnthraxToxin,
    InfernoFire,
    SpectreHowitzerShell,
    CountermeasureFlare,
    PointDefenseLaserBeam,
    WeaponLaserBeam,
    ParticleTrailRemnant,
    ParticleOrbitalLaser,
    ParticleConnectorLaser,
    FirewallSegment,
    RadarVanPing,
    MoneyCrate,
}
