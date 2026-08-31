//! Host HeightDieUpdate residual (die when altitude reaches target).
//!
//! C++: `HeightDieUpdate::update` kills when height-above-terrain ≤ TargetHeight
//! (optionally only while descending, after InitialDelay).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostHeightDieData {
    pub target_height_above_terrain: f32,
    pub only_when_descending: bool,
    pub earliest_death_frame: u32,
    pub last_height: f32,
    pub has_died: bool,
    pub active: bool,
}

impl Default for HostHeightDieData {
    fn default() -> Self {
        Self {
            target_height_above_terrain: 0.0,
            only_when_descending: true,
            earliest_death_frame: 0,
            last_height: f32::MAX,
            has_died: false,
            active: true,
        }
    }
}

impl HostHeightDieData {
    pub fn with_target(height: f32, only_descending: bool, earliest_frame: u32) -> Self {
        Self {
            target_height_above_terrain: height.max(0.0),
            only_when_descending: only_descending,
            earliest_death_frame: earliest_frame,
            last_height: f32::MAX,
            has_died: false,
            active: true,
        }
    }

    /// Returns true when object should die this frame.
    /// `height_above_terrain` is world Y - terrain Y (host Y-up).
    pub fn tick(&mut self, current_frame: u32, height_above_terrain: f32, contained: bool) -> bool {
        if !self.active || self.has_died {
            return false;
        }
        if contained {
            self.last_height = height_above_terrain;
            return false;
        }
        if current_frame < self.earliest_death_frame {
            self.last_height = height_above_terrain;
            return false;
        }
        let mut direction_ok = true;
        if self.only_when_descending && height_above_terrain >= self.last_height {
            direction_ok = false;
        }
        self.last_height = height_above_terrain;
        if direction_ok && height_above_terrain <= self.target_height_above_terrain {
            self.has_died = true;
            return true;
        }
        false
    }
}

/// Retail HeightDieUpdate INI (`TargetHeight`, `OnlyWhenMovingDown`, `InitialDelay` msec).
///
/// C++ `HeightDieUpdateModuleData::buildFieldParse` (HeightDieUpdate.cpp:43-61).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeightDieIni {
    pub target_height: f32,
    pub only_when_descending: bool,
    pub initial_delay_ms: u32,
    pub includes_structures: bool,
}

/// Common peels (target height, only_descending, initial delay msec).
/// Values are retail INI `TargetHeight` / `OnlyWhenMovingDown` / `InitialDelay`.
pub fn height_die_config_for_template(name: &str) -> Option<(f32, bool, u32)> {
    height_die_ini_for_template(name).map(|ini| {
        (
            ini.target_height,
            ini.only_when_descending,
            ini.initial_delay_ms,
        )
    })
}

/// C++ HeightDieUpdate INI lookup (TargetHeight + structures flag).
pub fn height_die_ini_for_template(name: &str) -> Option<HeightDieIni> {
    let n = name.to_ascii_lowercase();
    // FuelAir gas cloud before bomb/projectile peels.
    // Retail FuelAir / Aurora gas HeightDieUpdate TargetHeight = 15, not descending.
    if (n.contains("fuelair") && n.contains("gas"))
        || n.contains("aurorabombgas")
        || (n.contains("aurora") && n.contains("gas"))
    {
        return Some(HeightDieIni {
            target_height: 15.0,
            only_when_descending: false,
            initial_delay_ms: 0,
            includes_structures: false,
        });
    }
    // Retail NapalmBomb HeightDieUpdate TargetHeight = 1.0
    if n.contains("napalmbomb") || (n.contains("napalm") && n.contains("bomb")) {
        return Some(HeightDieIni {
            target_height: 1.0,
            only_when_descending: true,
            initial_delay_ms: 0,
            includes_structures: false,
        });
    }
    if n.contains("aurorabomb") || (n.contains("daisy") && n.contains("cutter")) {
        return Some(HeightDieIni {
            target_height: 10.0,
            only_when_descending: true,
            initial_delay_ms: 0,
            includes_structures: false,
        });
    }
    // Retail ScudStormMissile HeightDieUpdate before generic SCUDMissile peel.
    // TargetHeight 15, InitialDelay 1000ms, TargetHeightIncludesStructures Yes.
    // Missile objects only: the GLAScudStorm silo authors no HeightDieUpdate
    // (C++ HeightDieUpdate.cpp module table), so a name peel on the bare
    // structure name must not arm a die-at-rest module on the launcher.
    if n.contains("scudstorm") && n.contains("missile") {
        return Some(HeightDieIni {
            target_height: 15.0,
            only_when_descending: true,
            initial_delay_ms: 1000,
            includes_structures: true,
        });
    }
    if n.contains("scud") && n.contains("missile") {
        return Some(HeightDieIni {
            target_height: 10.0,
            only_when_descending: true,
            initial_delay_ms: 0,
            includes_structures: true,
        });
    }
    if n.contains("tomahawk") && n.contains("missile") {
        return Some(HeightDieIni {
            target_height: 10.0,
            only_when_descending: true,
            initial_delay_ms: 0,
            includes_structures: false,
        });
    }
    if n.contains("nuke") && n.contains("missile") && !n.contains("launcher") {
        return Some(HeightDieIni {
            target_height: 50.0,
            only_when_descending: true,
            initial_delay_ms: 1000,
            includes_structures: false,
        });
    }
    if n.contains("carpetbomb") || n.contains("moab") {
        return Some(HeightDieIni {
            target_height: 10.0,
            only_when_descending: true,
            initial_delay_ms: 0,
            includes_structures: false,
        });
    }
    if n.contains("fuelair") || n.contains("gasbomb") {
        return Some(HeightDieIni {
            target_height: 5.0,
            only_when_descending: true,
            initial_delay_ms: 0,
            includes_structures: false,
        });
    }
    if n.contains("projectile") || (n.contains("shell") && n.contains("artillery")) {
        return Some(HeightDieIni {
            target_height: 1.0,
            only_when_descending: true,
            initial_delay_ms: 0,
            includes_structures: false,
        });
    }
    // C++ attaches HeightDieUpdate only where a template AUTHORS the module
    // (HeightDieUpdate.cpp:43-61 field parse). The retail INIZH
    // WeaponObjects.ini census of `Behavior = HeightDieUpdate` owners is an
    // explicit list — no unit or vehicle whose name merely contains
    // "bomb"/"missile" authors the module (GLAVehicleBombTruck,
    // MissileDefender, PatriotMissile, TestMissileDefender all author NONE).
    // Peeling one onto them killed a Bomb Truck on its first tick
    // (ground HAT 0 <= target, `last_height` starts f32::MAX = descending),
    // before the disguise-as-vehicle ability could ever arm.
    //
    // Residual authored-module tokens (previously served by the removed
    // generic catch-alls, same residual values as before). Retail params
    // recorded for a future value-parity pass:
    //   AnthraxBomb/Gamma TH40 not-desc, ClusterMinesBomb TH60 not-desc,
    //   EMPPulseBomb TH15 not-desc, NukeBomb TH10 not-desc,
    //   CruiseMissile TH10 desc delay1000, NeutronMissile/SupW_NeutronMissile
    //   TH100 desc delay1000, PatriotMissileEMPHelper TH5 not-desc. The
    //   Rust-side `ChinaNuclearMissile` flight template keeps its residual
    //   peel (silo launchers stay excluded).
    if n.contains("anthraxbomb")
        || n.contains("clusterminesbomb")
        || n.contains("emppulsebomb")
        || n.contains("nukebomb")
    {
        return Some(HeightDieIni {
            target_height: 5.0,
            only_when_descending: true,
            initial_delay_ms: 0,
            includes_structures: false,
        });
    }
    if n.contains("cruisemissile")
        || n.contains("neutronmissile")
        || (n.contains("nuclearmissile") && !n.contains("launcher"))
        || n == "patriotmissileemphelper"
        || n == "testmissile"
    {
        return Some(HeightDieIni {
            target_height: 10.0,
            only_when_descending: true,
            initial_delay_ms: 0,
            includes_structures: false,
        });
    }
    None
}

/// C++ HeightDieUpdate.cpp:132-152: terrain + INI TargetHeight (+ structures).
pub fn height_die_target_world_y(
    terrain_height: f32,
    ini: &HeightDieIni,
    structure_height: f32,
) -> f32 {
    let mut extra = ini.target_height;
    if ini.includes_structures && structure_height > extra {
        extra = structure_height;
    }
    terrain_height + extra
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_die_on_descent() {
        let mut h = HostHeightDieData::with_target(10.0, true, 0);
        assert!(!h.tick(1, 50.0, false));
        assert!(!h.tick(2, 55.0, false)); // ascending
        assert!(!h.tick(3, 20.0, false)); // descending but above
        assert!(h.tick(4, 8.0, false)); // below target while descending
        assert!(!h.tick(5, 0.0, false)); // already died
    }

    #[test]
    fn height_die_ini_target_height_not_hardcoded_zero() {
        // C++ HeightDieUpdate.cpp:50 TargetHeight INI; NapalmBomb = 1.0.
        let ini = height_die_ini_for_template("NapalmBomb").expect("napalm ini");
        assert!((ini.target_height - 1.0).abs() < 0.001);
        assert!(ini.only_when_descending);
        let y = height_die_target_world_y(40.0, &ini, 0.0);
        assert!((y - 41.0).abs() < 0.001);
        assert!(height_die_ini_for_template("AmericaInfantryRanger").is_none());
    }

    #[test]
    fn height_die_peel_never_arms_silo_launcher_structures() {
        // C++ HeightDieUpdate modules live on the missile objects, not the
        // silo structures. A ground-rest launcher (hat 0) must not resolve a
        // die-at-rest module or update_ai removes the structure on tick one.
        assert!(height_die_ini_for_template("ChinaNuclearMissileLauncher").is_none());
        assert!(height_die_ini_for_template("GLAScudStorm").is_none());
        assert!(height_die_ini_for_template("Nuke_ChinaNuclearMissileLauncher").is_none());
        // The missiles themselves keep their retail peels.
        assert!(height_die_ini_for_template("ScudStormMissile").is_some());
        assert!(height_die_ini_for_template("ChinaNuclearMissile").is_some());
    }
}
