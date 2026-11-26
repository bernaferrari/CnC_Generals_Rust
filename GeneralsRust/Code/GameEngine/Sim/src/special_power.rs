//! Default AI targeting radii for named special powers.

/// Lookup the radius fallback by exact special-power template name.
pub fn fallback_radius_cursor(template_name: &str) -> Option<f32> {
    Some(match template_name {
        "SuperweaponDaisyCutter" => 170.0,
        "SuperweaponA10ThunderboltMissileStrike" => 50.0,
        "SuperweaponScudStorm" => 200.0,
        "SuperweaponParticleUplinkCannon" => 0.0,
        "SpecialPowerSpySatellite" => 300.0,
        "SpecialPowerRadarVanScan" => 150.0,
        "SuperweaponCarpetBomb" => 100.0,
        "SuperweaponClusterMines" => 100.0,
        "SuperweaponParadropAmerica" => 50.0,
        "SpecialPowerSpyDrone" => 250.0,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::fallback_radius_cursor;

    #[test]
    fn radius_cursor_fallback_matches_template_name_exactly() {
        assert_eq!(
            fallback_radius_cursor("SuperweaponDaisyCutter"),
            Some(170.0)
        );
        assert_eq!(
            fallback_radius_cursor("SuperweaponParticleUplinkCannon"),
            Some(0.0)
        );
        assert_eq!(fallback_radius_cursor("superweapondaisycutter"), None);
    }
}
