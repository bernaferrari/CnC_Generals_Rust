use super::*;

/// C++ Weapon.ini HistoricBonus residual peels.
#[derive(Debug, Clone, PartialEq)]
pub struct HostHistoricBonusPeel {
    /// Frames window (HistoricBonusTime msec → frames @ 30 FPS).
    pub time_frames: u32,
    pub count: i32,
    pub radius: f32,
    /// Bonus weapon template name (FirestormSmallCreationWeapon residual).
    pub bonus_weapon: String,
}

impl Default for HostHistoricBonusPeel {
    fn default() -> Self {
        Self {
            time_frames: 0,
            count: 0,
            radius: 0.0,
            bonus_weapon: String::new(),
        }
    }
}

impl HostHistoricBonusPeel {
    pub fn is_active(&self) -> bool {
        self.count > 0 && self.time_frames > 0 && !self.bonus_weapon.is_empty()
    }

    pub fn is_black_napalm_bonus(&self) -> bool {
        let n = self.bonus_weapon.to_ascii_lowercase();
        n.contains("black")
    }
}

/// Resolve HistoricBonus peels from leftover Weapon.ini store / seeds.
///
/// Leftover `resolve_historic_bonus_weapon` is RIGHT: Weak → stored
/// `historic_bonus_weapon_name` → store lookup. Live must not substitute
/// hardcoded firestorm seeds when the Weak is dead (INI order).
pub fn host_historic_bonus_for_weapon_name(name: &str) -> HostHistoricBonusPeel {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|wt| HostHistoricBonusPeel {
                time_frames: wt.historic_bonus_time,
                count: wt.historic_bonus_count,
                radius: wt.historic_bonus_radius.max(0.0),
                bonus_weapon: leftover_historic_bonus_weapon_name(wt),
            })
    })
    .ok()
    .flatten();
    if let Some(p) = from_store {
        // Store authored HistoricBonusTime/Count: keep leftover name (or empty).
        // Do not seed Firestorm* over HistoricBonusWeapon = None / custom name.
        if p.count > 0 && p.time_frames > 0 {
            return p;
        }
    }
    seed_historic_bonus_for(name)
}

/// Leftover resolve order without re-entering the store lock: live Weak, else
/// authored `historic_bonus_weapon_name` (C++ `INI::parseWeaponTemplate`).
fn leftover_historic_bonus_weapon_name(wt: &WeaponTemplate) -> String {
    if let Some(arc) = wt
        .historic_bonus_weapon
        .as_ref()
        .and_then(|weak| weak.upgrade())
    {
        if !arc.name.is_empty() && !arc.name.eq_ignore_ascii_case(&wt.name) {
            return arc.name.clone();
        }
        return String::new();
    }
    let named = wt.historic_bonus_weapon_name.trim();
    if !named.is_empty()
        && !named.eq_ignore_ascii_case("None")
        && !named.eq_ignore_ascii_case(&wt.name)
    {
        return named.to_string();
    }
    String::new()
}

pub(super) fn seed_historic_bonus_weapon_for(name: &str) -> String {
    let n = name.to_ascii_lowercase();
    if n.contains("upgraded") || n.contains("black") {
        "BlackNapalmFirestormSmallCreationWeapon".into()
    } else {
        "FirestormSmallCreationWeapon".into()
    }
}

pub(super) fn seed_historic_bonus_for(name: &str) -> HostHistoricBonusPeel {
    let n = name.to_ascii_lowercase();
    // InfernoCannonGun: Time 3000ms→90f, Count 3, Radius 20
    if n.contains("infernocannon") {
        return HostHistoricBonusPeel {
            time_frames: 90,
            count: 3,
            radius: 20.0,
            bonus_weapon: seed_historic_bonus_weapon_for(name),
        };
    }
    // MiG NapalmMissile: Count 8, Radius 100, Time 3000ms
    if n.contains("napalmmissile") || (n.contains("mig") && n.contains("napalm")) {
        return HostHistoricBonusPeel {
            time_frames: 90,
            count: 8,
            radius: 100.0,
            bonus_weapon: seed_historic_bonus_weapon_for(name),
        };
    }
    HostHistoricBonusPeel::default()
}

/// C++ Weapon.ini LeechRangeWeapon residual peel.
///
/// Once a leech weapon has entered pre-attack / fired once at proper range,
/// max-range is waived for the remainder of the attack cycle (AI chase residual).
pub fn host_leech_range_weapon_for_weapon_name(name: &str) -> bool {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|wt| wt.leech_range_weapon)
    })
    .ok()
    .flatten();
    if let Some(v) = from_store {
        return v;
    }
    seed_leech_range_weapon_for(name)
}

pub(super) fn seed_leech_range_weapon_for(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    // Suicide / melee / knife residual weapons.
    if n.contains("terrorist")
        || n.contains("carbomb")
        || n.contains("demotrap")
        || n.contains("suicide")
        || n.contains("knife")
        || n.contains("melee")
        || n.contains("burton")
        || n.contains("rangerflash")
        || n.contains("stinger")
    {
        return true;
    }
    false
}

/// C++ Weapon.ini ScaleWeaponSpeed / MinWeaponSpeed residual peels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostWeaponSpeedPeel {
    pub weapon_speed: f32,
    pub min_weapon_speed: f32,
    pub scale_weapon_speed: bool,
    pub attack_range: f32,
    pub min_attack_range: f32,
}

impl Default for HostWeaponSpeedPeel {
    fn default() -> Self {
        Self {
            weapon_speed: 0.0,
            min_weapon_speed: 0.0,
            scale_weapon_speed: false,
            attack_range: 0.0,
            min_attack_range: 0.0,
        }
    }
}

/// Resolve WeaponSpeed / MinWeaponSpeed / ScaleWeaponSpeed / ranges from store.
pub fn host_weapon_speed_peel_for_weapon_name(name: &str) -> HostWeaponSpeedPeel {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store.find_weapon_template(name).map(|wt| {
            let weapon_speed = if wt.weapon_speed >= 999_999.0 {
                0.0
            } else {
                wt.weapon_speed.max(0.0)
            };
            let min_weapon_speed = if wt.min_weapon_speed >= 999_999.0 {
                weapon_speed
            } else {
                wt.min_weapon_speed.max(0.0)
            };
            HostWeaponSpeedPeel {
                weapon_speed,
                min_weapon_speed,
                scale_weapon_speed: wt.is_scale_weapon_speed,
                attack_range: wt.attack_range.max(0.0),
                min_attack_range: wt.minimum_attack_range.max(0.0),
            }
        })
    })
    .ok()
    .flatten();
    if let Some(p) = from_store {
        // Prefer store when it has meaningful speed or scale flag.
        if p.weapon_speed > 0.0 || p.scale_weapon_speed {
            return p;
        }
    }
    seed_weapon_speed_peel_for(name)
}

pub(super) fn seed_weapon_speed_peel_for(name: &str) -> HostWeaponSpeedPeel {
    let n = name.to_ascii_lowercase();
    // AmericaFireBaseHowitzer / StrategyCenter artillery lob residual.
    if n.contains("firebase") || n.contains("howitzer") {
        return HostWeaponSpeedPeel {
            weapon_speed: 300.0,
            min_weapon_speed: 75.0,
            scale_weapon_speed: true,
            attack_range: 375.0,
            min_attack_range: 50.0,
        };
    }
    if n.contains("artillery") || n.contains("strategycenter") {
        return HostWeaponSpeedPeel {
            weapon_speed: 300.0,
            min_weapon_speed: 75.0,
            scale_weapon_speed: true,
            attack_range: 400.0,
            min_attack_range: 50.0,
        };
    }
    if n.contains("scud") && n.contains("weapon") && !n.contains("damage") {
        return HostWeaponSpeedPeel {
            weapon_speed: 150.0,
            min_weapon_speed: 75.0,
            scale_weapon_speed: true,
            attack_range: 250.0,
            min_attack_range: 0.0,
        };
    }
    HostWeaponSpeedPeel::default()
}

/// C++ DumbProjectileBehavior ScaleWeaponSpeed residual:
/// `speed = min + ratio * (max - min)` where
/// `ratio = (range2d - minRange) / (maxRange - minRange)`.
pub fn host_scaled_weapon_speed(peel: &HostWeaponSpeedPeel, range_2d: f32) -> f32 {
    if !peel.scale_weapon_speed {
        return peel.weapon_speed;
    }
    let max_r = peel.attack_range;
    let min_r = peel.min_attack_range;
    let span = max_r - min_r;
    if span <= 1e-6 {
        return peel.weapon_speed;
    }
    let ratio = (range_2d - min_r) / span;
    peel.min_weapon_speed + ratio * (peel.weapon_speed - peel.min_weapon_speed)
}
