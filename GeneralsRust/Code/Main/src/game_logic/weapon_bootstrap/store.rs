use super::*;
use crate::game_logic::host_mines::{
    DOZER_MINE_CLEAR_CLIP_RELOAD_FRAMES, DOZER_MINE_CLEAR_PRE_ATTACK_FRAMES,
    DOZER_MINE_CLEAR_RANGE, WORKER_MINE_CLEAR_DELAY_FRAMES, WORKER_MINE_CLEAR_PRE_ATTACK_FRAMES,
};

pub(super) static BOOTSTRAP_ATTEMPTED: AtomicBool = AtomicBool::new(false);
pub(super) static SEED_COMPLETE: AtomicBool = AtomicBool::new(false);
pub fn ensure_host_weapon_store() -> usize {
    // create_object resolves weapons per placement. After start_new_game the
    // store is already filled; do not walk seed tables or Weapon.ini again.
    if SEED_COMPLETE.load(Ordering::Relaxed) {
        return 0;
    }
    if let Err(e) = gamelogic::initialize_weapon_store() {
        log::warn!("WeaponStore init failed during host bootstrap: {e}");
        return 0;
    }

    let mut added = 0usize;

    // Prefer real INI data when extracted game data is on disk (once / until ranger present).
    if !store_has(RANGER_PRIMARY_WEAPON)
        && (!BOOTSTRAP_ATTEMPTED.swap(true, Ordering::Relaxed) || !store_has(RANGER_PRIMARY_WEAPON))
    {
        added += try_load_weapon_ini_from_disk();
    }
    BOOTSTRAP_ATTEMPTED.store(true, Ordering::Relaxed);

    // Always fill gaps for known host weapons (units + base-defense residual).
    // seed_known_host_weapons skips names already present in the store.
    added += seed_known_host_weapons();
    SEED_COMPLETE.store(true, Ordering::Relaxed);
    added
}
pub(super) fn store_has(name: &str) -> bool {
    with_weapon_store(|store| store.find_weapon_template(name).is_some()).unwrap_or(false)
}

pub(super) fn try_load_weapon_ini_from_disk() -> usize {
    let mut total = 0usize;
    for path in weapon_ini_candidate_paths() {
        if !path.is_file() {
            continue;
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let n =
                    crate::assets::ini_template_loader::register_weapons_from_ini_text(&content);
                if n > 0 {
                    log::info!(
                        "Host WeaponStore: loaded {} weapon templates from {}",
                        n,
                        path.display()
                    );
                    total += n;
                    // One full Weapon.ini is enough; further copies are overrides.
                    break;
                }
            }
            Err(e) => {
                log::debug!("Host WeaponStore: cannot read {}: {e}", path.display());
            }
        }
    }
    total
}

pub(super) fn weapon_ini_candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // CWD-relative (repo root when tests run from GeneralsRust or workspace).
    let relative = [
        "windows_game/extracted_big_files/INIZH/Data/INI/Weapon.ini",
        "windows_game/extracted_big_files_v2/INIZH/Data/INI/Weapon.ini",
        "Data/INI/Weapon.ini",
        "INIZH/Data/INI/Weapon.ini",
    ];
    for r in relative {
        paths.push(PathBuf::from(r));
    }

    // Walk up from CARGO_MANIFEST_DIR (Main crate) toward repo root.
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let mut dir = PathBuf::from(manifest);
        for _ in 0..6 {
            for r in relative {
                paths.push(dir.join(r));
            }
            if !dir.pop() {
                break;
            }
        }
    }

    // Dedup while preserving order.
    let mut seen = std::collections::HashSet::new();
    paths.retain(|p| seen.insert(p.clone()));
    paths
}

/// Seed minimal real-ish stats for host golden units when INI is unavailable.
///
/// Values match retail Weapon.ini entries used by those units (damage/range).
/// Delay is stored in logic frames (30 FPS) after msec conversion.

pub(super) fn seed_death_type_for(
    name: &str,
    damage_type: gamelogic::damage::DamageType,
) -> gamelogic::damage::DeathType {
    use gamelogic::damage::{DamageType as Dmg, DeathType as Dth};
    let n = name.to_ascii_lowercase();
    if n.contains("terrorist") || n.contains("suicide") || n.contains("demo_trap") {
        return Dth::Suicided;
    }
    if n.contains("laser") {
        return Dth::Lasered;
    }
    if n.contains("toxin") || n.contains("anthrax") || n.contains("poison") {
        return Dth::Poisoned;
    }
    if n.contains("flame") || n.contains("dragon") || n.contains("napalm") || n.contains("inferno")
    {
        return Dth::Burned;
    }
    if n.contains("bomb")
        || n.contains("scud")
        || n.contains("missile")
        || n.contains("rocket")
        || n.contains("aurora")
        || n.contains("grenade")
    {
        return Dth::Exploded;
    }
    match damage_type {
        Dmg::Flame => Dth::Burned,
        Dmg::Laser => Dth::Lasered,
        Dmg::Poison => Dth::Poisoned,
        Dmg::Explosion | Dmg::AuroraBomb | Dmg::LandMine | Dmg::MolotovCocktail => Dth::Exploded,
        Dmg::Radiation => Dth::Detonated,
        _ => Dth::Normal,
    }
}

pub(super) fn seed_damage_type_for(name: &str, weapon_speed: f32) -> gamelogic::damage::DamageType {
    use gamelogic::damage::DamageType as D;
    let n = name.to_ascii_lowercase();
    if n.contains("laser") || n.contains("point_defense") || n.contains("pointdefense") {
        return D::Laser;
    }
    if n.contains("flame")
        || n.contains("dragon")
        || n.contains("napalm")
        || n.contains("inferno")
        || n.contains("fire")
    {
        return D::Flame;
    }
    if n.contains("toxin") || n.contains("anthrax") || n.contains("poison") {
        return D::Poison;
    }
    if n.contains("neutron") || n.contains("nuke") || n.contains("radiation") {
        return D::Radiation;
    }
    if n.contains("buildingclearer") || n.contains("killgarrison") || n.contains("clearbuilding") {
        return D::KillGarrisoned;
    }
    if n.contains("emp") || n.contains("microwave") {
        return D::Microwave;
    }
    if n.contains("vehiclepilot")
        || n.contains("pilotsniper")
        || n.contains("killpilot")
        || n.contains("kill_pilot")
    {
        return D::KillPilot;
    }
    if n.contains("sniper") || n.contains("jarmen") {
        return D::Sniper;
    }
    if n.contains("gattling") || n.contains("gatling") {
        return D::Gattling;
    }
    // C++ Weapon.ini DamageType=SURRENDER residual: RangerFlashBangGrenadeWeapon
    // must keep Surrender (VoiceSubdue / VoiceClearBuilding path), so the
    // flashbang peel runs BEFORE the generic grenade→Explosion name match.
    if n.contains("flashbang") {
        return D::Surrender;
    }
    if n.contains("bomb")
        || n.contains("scud")
        || n.contains("missile")
        || n.contains("rocket")
        || n.contains("tomahawk")
        || n.contains("aurora")
        || n.contains("grenade")
        || n.contains("demo")
    {
        return D::Explosion;
    }
    // C++ Weapon.ini DamageType=DISARM residual: mine-clearing detail weapons
    // (DozerMineDisarmingWeapon / WorkerMineDisarmingWeapon) must seed Disarm
    // so the ground-fire path destroys armed mines via disarm_mine_safe
    // instead of dealing 1 HP SmallArms.
    if crate::game_logic::weapon_bootstrap::host_weapon_is_disarm_damage(name) {
        return D::Disarm;
    }
    if weapon_speed >= 999_000.0 || weapon_speed <= 0.0 {
        if n.contains("gun") || n.contains("rifle") || n.contains("machine") {
            return D::SmallArms;
        }
        // many instant hits are small arms hitscan residual in host seeds
        return D::SmallArms;
    }
    if n.contains("tankgun") || n.contains("tank_gun") || n.contains("cannon") {
        return D::ArmorPiercing;
    }
    D::SmallArms
}

pub(super) fn seed_known_host_weapons() -> usize {
    let seeds = [
        // AmericaInfantryRanger PRIMARY — PrimaryDamage 5, AttackRange 100,
        // DelayBetweenShots 100ms → 3 frames @ 30 FPS.
        SeedWeapon {
            name: RANGER_PRIMARY_WEAPON,
            primary_damage: 5.0,
            attack_range: 100.0,
            delay_frames: 3,
            clip_size: 3,
            weapon_speed: 999_999.0,
        },
        // GLAInfantryRebel PRIMARY
        SeedWeapon {
            name: GLA_REBEL_PRIMARY_WEAPON,
            primary_damage: 5.0,
            attack_range: 100.0,
            delay_frames: 3,
            clip_size: 3,
            weapon_speed: 999_999.0,
        },
        // ChinaInfantryRedguard PRIMARY — PrimaryDamage 15, Delay 1000ms → 30 frames
        SeedWeapon {
            name: REDGUARD_PRIMARY_WEAPON,
            primary_damage: 15.0,
            attack_range: 100.0,
            delay_frames: 30,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // ChinaInfantryTankHunter PRIMARY — PrimaryDamage 40, range 175, Delay 1000ms → 30 frames
        SeedWeapon {
            name: TANK_HUNTER_PRIMARY_WEAPON,
            primary_damage: 40.0,
            attack_range: 175.0,
            delay_frames: 30,
            clip_size: 0,
            weapon_speed: 600.0,
        },
        // AmericaVehicleHumvee PRIMARY — damage 10, range 150, delay 200ms → 6 frames
        SeedWeapon {
            name: HUMVEE_PRIMARY_WEAPON,
            primary_damage: 10.0,
            attack_range: 150.0,
            delay_frames: 6,
            clip_size: 0,
            weapon_speed: 600.0,
        },
        // AmericaInfantryRanger SECONDARY — RangerFlashBangGrenadeWeapon
        // PrimaryDamage 35, AttackRange 175, ClipReload 2000ms → 60 frames
        SeedWeapon {
            name: RANGER_SECONDARY_WEAPON,
            primary_damage: 35.0,
            attack_range: 175.0,
            delay_frames: 60,
            clip_size: 1,
            weapon_speed: 120.0,
        },
        // AmericaVehicleHumvee SECONDARY — HumveeMissileWeapon
        // PrimaryDamage 30, AttackRange 150, Delay 1000ms → 30 frames
        SeedWeapon {
            name: HUMVEE_SECONDARY_WEAPON,
            primary_damage: 30.0,
            attack_range: 150.0,
            delay_frames: 30,
            clip_size: 1,
            weapon_speed: 600.0,
        },
        // AmericaPatriotBattery PRIMARY — PrimaryDamage 30, AttackRange 225,
        // ClipReload 2000ms residual cadence → 60 frames @ 30 FPS.
        SeedWeapon {
            name: PATRIOT_PRIMARY_WEAPON,
            primary_damage: 30.0,
            attack_range: 225.0,
            delay_frames: 60,
            clip_size: 4,
            weapon_speed: 600.0,
        },
        // AmericaPatriotBattery SECONDARY AA — PrimaryDamage 25, AttackRange 350.
        SeedWeapon {
            name: PATRIOT_SECONDARY_WEAPON,
            primary_damage: 25.0,
            attack_range: 350.0,
            delay_frames: 60,
            clip_size: 4,
            weapon_speed: 600.0,
        },
        // GLA Stinger Site residual (soldier PRIMARY) — PrimaryDamage 20, range 225,
        // ClipReload 2000ms → 60 frames. SPAWNS_ARE_THE_WEAPONS structure residual.
        SeedWeapon {
            name: STINGER_PRIMARY_WEAPON,
            primary_damage: 20.0,
            attack_range: 225.0,
            delay_frames: 60,
            clip_size: 1,
            weapon_speed: 750.0,
        },
        // GLA Stinger Site residual (soldier SECONDARY AA) — PrimaryDamage 30, range 400.
        SeedWeapon {
            name: STINGER_SECONDARY_WEAPON,
            primary_damage: 30.0,
            attack_range: 400.0,
            delay_frames: 60,
            clip_size: 1,
            weapon_speed: 600.0,
        },
        // ChinaGattlingCannon PRIMARY — PrimaryDamage 10, AttackRange 225,
        // DelayBetweenShots 250ms → 8 frames @ 30 FPS. Instant hit.
        SeedWeapon {
            name: GATTLING_BUILDING_PRIMARY_WEAPON,
            primary_damage: 10.0,
            attack_range: 225.0,
            delay_frames: 8,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // ChinaGattlingCannon SECONDARY AA — PrimaryDamage 5, AttackRange 400,
        // DelayBetweenShots 250ms → 8 frames. Continuous-fire ramp shares structure residual.
        SeedWeapon {
            name: GATTLING_BUILDING_SECONDARY_WEAPON,
            primary_damage: 5.0,
            attack_range: 400.0,
            delay_frames: 8,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // ChinaVehicleNukeCannon PRIMARY — NukeCannonGun residual seed.
        // PrimaryDamage 400 retail shell; host residual area path uses 400/50 + 20/60.
        SeedWeapon {
            name: NUKE_CANNON_PRIMARY_WEAPON,
            primary_damage: 400.0,
            attack_range: 350.0,
            delay_frames: 300,
            clip_size: 0,
            weapon_speed: 200.0,
        },
        // NukeCannonNeutronWeapon SECONDARY — PrimaryDamage 1 (blast does work),
        // AttackRange 350, Delay 10000ms → 300 frames. Blast via host residual.
        SeedWeapon {
            name: NUKE_CANNON_NEUTRON_WEAPON,
            primary_damage: 1.0,
            attack_range: 350.0,
            delay_frames: 300,
            clip_size: 0,
            weapon_speed: 200.0,
        },
        // PaladinPointDefenseLaser — PrimaryDamage 100, AttackRange 65,
        // DelayBetweenShots 1000ms → 30 frames. Instant laser.
        SeedWeapon {
            name: PALADIN_POINT_DEFENSE_LASER,
            primary_damage: 100.0,
            attack_range: 65.0,
            delay_frames: 30,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // AvengerPointDefenseLaserOne — PrimaryDamage 100, AttackRange 100,
        // DelayBetweenShots 500ms → 15 frames.
        SeedWeapon {
            name: AVENGER_POINT_DEFENSE_LASER,
            primary_damage: 100.0,
            attack_range: 100.0,
            delay_frames: 15,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // CrusaderTankGun PRIMARY — PrimaryDamage 60, AttackRange 150,
        // DelayBetweenShots 2000ms → 60 frames.
        SeedWeapon {
            name: CRUSADER_TANK_GUN,
            primary_damage: 60.0,
            attack_range: 150.0,
            delay_frames: 60,
            clip_size: 0,
            weapon_speed: 400.0,
        },
        // PaladinTankGun PRIMARY — same residual stats as CrusaderTankGun.
        SeedWeapon {
            name: PALADIN_TANK_GUN,
            primary_damage: 60.0,
            attack_range: 150.0,
            delay_frames: 60,
            clip_size: 0,
            weapon_speed: 300.0,
        },
        // Lazr_CrusaderTankGun PRIMARY — PrimaryDamage 80, AttackRange 150,
        // DelayBetweenShots 2000ms → 60 frames. Instant laser residual.
        SeedWeapon {
            name: LAZR_CRUSADER_TANK_GUN,
            primary_damage: 80.0,
            attack_range: 150.0,
            delay_frames: 60,
            clip_size: 0,
            weapon_speed: 99999.0,
        },
        // Lazr_PaladinTankGun PRIMARY — PrimaryDamage 70, AttackRange 150,
        // DelayBetweenShots 1000ms → 30 frames.
        SeedWeapon {
            name: LAZR_PALADIN_TANK_GUN,
            primary_damage: 70.0,
            attack_range: 150.0,
            delay_frames: 30,
            clip_size: 0,
            weapon_speed: 99999.0,
        },
        // Lazr_PatriotMissileWeapon PRIMARY — PrimaryDamage 40, Range 225,
        // ClipReload residual cadence 2000ms → 60 frames.
        SeedWeapon {
            name: LAZR_PATRIOT_PRIMARY_WEAPON,
            primary_damage: 40.0,
            attack_range: 225.0,
            delay_frames: 60,
            clip_size: 3,
            weapon_speed: 999999.0,
        },
        // Lazr_PatriotMissileWeaponAir residual secondary — PrimaryDamage 35, Range 350.
        SeedWeapon {
            name: LAZR_PATRIOT_SECONDARY_WEAPON,
            primary_damage: 35.0,
            attack_range: 350.0,
            delay_frames: 60,
            clip_size: 4,
            weapon_speed: 999999.0,
        },
        // SupW_PatriotMissileWeapon PRIMARY — PrimaryDamage 15, Range 275,
        // EMP detonation residual (DISABLED_EMP r10 / 10s).
        SeedWeapon {
            name: SUPW_PATRIOT_PRIMARY_WEAPON,
            primary_damage: 15.0,
            attack_range: 275.0,
            delay_frames: 60,
            clip_size: 4,
            weapon_speed: 600.0,
        },
        // SupW_PatriotMissileWeaponAir residual secondary — PrimaryDamage 30, Range 400.
        SeedWeapon {
            name: SUPW_PATRIOT_SECONDARY_WEAPON,
            primary_damage: 30.0,
            attack_range: 400.0,
            delay_frames: 60,
            clip_size: 4,
            weapon_speed: 400.0,
        },
        // TunnelNetworkGun PRIMARY — PrimaryDamage 15, Range 175,
        // DelayBetweenShots 250ms → 8 frames.
        SeedWeapon {
            name: TUNNEL_NETWORK_GUN,
            primary_damage: 15.0,
            attack_range: 175.0,
            delay_frames: 8,
            clip_size: 0,
            weapon_speed: 600.0,
        },
        // AvengerTargetDesignator PRIMARY — STATUS paint residual (0 HP dmg).
        // PrimaryDamage in retail is duration ms; host residual damage=0.
        SeedWeapon {
            name: AVENGER_TARGET_DESIGNATOR,
            primary_damage: 0.0,
            attack_range: 200.0,
            delay_frames: 6,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // AvengerAirLaserOne residual secondary AA — PrimaryDamage 10, Range 300,
        // Delay 200ms → 6 frames. Anti-air only (seeded separately below).
        SeedWeapon {
            name: AVENGER_AIR_LASER,
            primary_damage: 10.0,
            attack_range: 300.0,
            delay_frames: 6,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // HumveeMissileWeaponAir tertiary residual — PrimaryDamage 50, Range 320,
        // Delay+ClipReload residual cycle → 90 frames. Anti-air only.
        SeedWeapon {
            name: HUMVEE_MISSILE_WEAPON_AIR,
            primary_damage: 50.0,
            attack_range: 320.0,
            delay_frames: 90,
            clip_size: 1,
            weapon_speed: 600.0,
        },
        // InfernoCannonGun PRIMARY — PrimaryDamage 30, AttackRange 300,
        // DelayBetweenShots 4000ms → 120 frames. FireFieldSmall residual on impact.
        SeedWeapon {
            name: INFERNO_CANNON_PRIMARY_WEAPON,
            primary_damage: 30.0,
            attack_range: 300.0,
            delay_frames: 120,
            clip_size: 0,
            weapon_speed: 250.0,
        },
        // AuroraBombWeapon PRIMARY — PrimaryDamage 400, AttackRange 300,
        // ClipReload 5000ms → 150 frames. Delayed dive residual applies AOE.
        SeedWeapon {
            name: AURORA_BOMB_PRIMARY_WEAPON,
            primary_damage: 400.0,
            attack_range: 300.0,
            delay_frames: 150,
            clip_size: 1,
            weapon_speed: 99999.0,
        },
        // AirF_AuroraBombWeapon — tiny primary; FuelAir detonation residual.
        SeedWeapon {
            name: AIRF_AURORA_BOMB_PRIMARY_WEAPON,
            primary_damage: 2.0,
            attack_range: 300.0,
            delay_frames: 150,
            clip_size: 1,
            weapon_speed: 99999.0,
        },
        // SupW_AuroraFuelBombWeapon — FuelAir residual path.
        SeedWeapon {
            name: SUPW_AURORA_FUEL_BOMB_WEAPON,
            primary_damage: 400.0,
            attack_range: 300.0,
            delay_frames: 150,
            clip_size: 1,
            weapon_speed: 99999.0,
        },
        // StealthJetMissileWeapon PRIMARY — PrimaryDamage 100, AttackRange 220,
        // Delay 200ms → 6 frames. Bunker-buster residual on impact when upgraded.
        SeedWeapon {
            name: STEALTH_JET_MISSILE_WEAPON,
            primary_damage: 100.0,
            attack_range: 220.0,
            delay_frames: 6,
            clip_size: 2,
            weapon_speed: 1000.0,
        },
        // NapalmMissileWeapon PRIMARY — PrimaryDamage 75, Range 320, min 80,
        // Delay 300ms → 9 frames. ClipSize 2. FireField residual on impact.
        SeedWeapon {
            name: NAPALM_MISSILE_WEAPON,
            primary_damage: 75.0,
            attack_range: 320.0,
            delay_frames: 9,
            clip_size: 2,
            weapon_speed: 1000.0,
        },
        // BlackNapalmMissileWeapon — same primary, upgraded secondary/field residual.
        SeedWeapon {
            name: BLACK_NAPALM_MISSILE_WEAPON,
            primary_damage: 75.0,
            attack_range: 320.0,
            delay_frames: 9,
            clip_size: 2,
            weapon_speed: 1000.0,
        },
        // Nuke_MiGMissileWeapon — PrimaryDamage 100 residual.
        SeedWeapon {
            name: NUKE_MIG_MISSILE_WEAPON,
            primary_damage: 100.0,
            attack_range: 320.0,
            delay_frames: 9,
            clip_size: 2,
            weapon_speed: 1000.0,
        },
        // Nuke_NukeMissileWeapon — PrimaryDamage 150 residual.
        SeedWeapon {
            name: NUKE_NUKE_MISSILE_WEAPON,
            primary_damage: 150.0,
            attack_range: 320.0,
            delay_frames: 9,
            clip_size: 2,
            weapon_speed: 1000.0,
        },
        // FireBaseHowitzerGun PRIMARY — PrimaryDamage 75, Range 275, min 50,
        // Delay 2000ms → 60 frames.
        SeedWeapon {
            name: FIRE_BASE_HOWITZER_WEAPON,
            primary_damage: 75.0,
            attack_range: 275.0,
            delay_frames: 60,
            clip_size: 0,
            weapon_speed: 300.0,
        },
        // RaptorJetMissileWeapon PRIMARY — PrimaryDamage 100, Range 320, min 100,
        // Delay 150ms → 5 frames. ClipSize 4 honesty.
        SeedWeapon {
            name: RAPTOR_JET_MISSILE_WEAPON,
            primary_damage: 100.0,
            attack_range: 320.0,
            delay_frames: 5,
            clip_size: 4,
            weapon_speed: 1000.0,
        },
        // AirF_RaptorJetMissileWeapon — PrimaryDamage 125, Range 350, Delay 75ms → 3 frames.
        SeedWeapon {
            name: AIRF_RAPTOR_JET_MISSILE_WEAPON,
            primary_damage: 125.0,
            attack_range: 350.0,
            delay_frames: 3,
            clip_size: 6,
            weapon_speed: 1000.0,
        },
        // BattleDroneMachineGun — PrimaryDamage 1, Range 110, Delay 100ms → 3 frames.
        SeedWeapon {
            name: BATTLE_DRONE_MACHINE_GUN,
            primary_damage: 1.0,
            attack_range: 110.0,
            delay_frames: 3,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // MicrowaveTankBuildingClearer — PrimaryDamage 1 (kills 1 garrisoned unit),
        // AttackRange 125, DelayBetweenShots 100ms → 3 frames @ 30 FPS.
        // KILL_GARRISONED residual via host combat path.
        SeedWeapon {
            name: MICROWAVE_BUILDING_CLEARER_WEAPON,
            primary_damage: 1.0,
            attack_range: 125.0,
            delay_frames: 3,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // Comanche20mmCannonWeapon PRIMARY — PrimaryDamage 6, AttackRange 200,
        // DelayBetweenShots 100ms → 3 frames @ 30 FPS.
        SeedWeapon {
            name: COMANCHE_PRIMARY_WEAPON,
            primary_damage: 6.0,
            attack_range: 200.0,
            delay_frames: 3,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // HelixMinigunWeapon PRIMARY — PrimaryDamage 6, AttackRange 115,
        // DelayBetweenShots 100ms → 3 frames @ 30 FPS. Intended-only residual.
        SeedWeapon {
            name: HELIX_MINIGUN_WEAPON,
            primary_damage: 6.0,
            attack_range: 115.0,
            delay_frames: 3,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // ComancheAntiTankMissileWeapon residual SECONDARY — PrimaryDamage 50,
        // radius dual residual via host (50/5 + 30/25), Range 200, Delay 500ms → 15 frames.
        SeedWeapon {
            name: COMANCHE_ANTITANK_WEAPON,
            primary_damage: 50.0,
            attack_range: 200.0,
            delay_frames: 15,
            clip_size: 4,
            weapon_speed: 99999.0,
        },
        // ComancheRocketPodWeapon residual TERTIARY after upgrade.
        // PrimaryDamage 30, AttackRange 200, Delay 200ms → 6 frames.
        // Area damage applied by host residual (primary/secondary rings).
        SeedWeapon {
            name: COMANCHE_ROCKET_POD_WEAPON,
            primary_damage: 30.0,
            attack_range: 200.0,
            delay_frames: 6,
            clip_size: 20,
            weapon_speed: 99999.0,
        },
        // SentryDroneGun PRIMARY after PLAYER_UPGRADE — PrimaryDamage 8, Range 150,
        // Delay 200ms → 6 frames.
        SeedWeapon {
            name: SENTRY_DRONE_GUN_WEAPON,
            primary_damage: 8.0,
            attack_range: 150.0,
            delay_frames: 6,
            clip_size: 0,
            weapon_speed: 600.0,
        },
        // USAPathfinderSniperRifle PRIMARY — PrimaryDamage 100, Range 300,
        // Delay 2000ms → 60 frames.
        SeedWeapon {
            name: PATHFINDER_SNIPER_WEAPON,
            primary_damage: 100.0,
            attack_range: 300.0,
            delay_frames: 60,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // HellfireMissileWeapon PRIMARY — PrimaryDamage 40, Range 150,
        // Delay+ClipReload ~3000ms → 90 frames.
        SeedWeapon {
            name: HELLFIRE_MISSILE_WEAPON,
            primary_damage: 40.0,
            attack_range: 150.0,
            delay_frames: 90,
            clip_size: 1,
            weapon_speed: 600.0,
        },
        // GLA Angry Mob residual aggregate fire — PrimaryDamage 20 (5 members × 4),
        // AttackRange 100, Delay 250ms → 8 frames. update_angry_mobs deals real residual.
        SeedWeapon {
            name: ANGRY_MOB_RESIDUAL_WEAPON,
            primary_damage: 20.0,
            attack_range: 100.0,
            delay_frames: 8,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // GLA Rocket Buggy BuggyRocketWeapon — PrimaryDamage 20, Range 300,
        // Delay 200ms → 6 frames, clip 6. Min range / splash residual via host.
        SeedWeapon {
            name: BUGGY_ROCKET_WEAPON,
            primary_damage: 20.0,
            attack_range: 300.0,
            delay_frames: 6,
            clip_size: 6,
            weapon_speed: 600.0,
        },
        // BuggyRocketWeaponUpgraded — same damage, clip 12 residual.
        SeedWeapon {
            name: BUGGY_ROCKET_WEAPON_UPGRADED,
            primary_damage: 20.0,
            attack_range: 300.0,
            delay_frames: 6,
            clip_size: 12,
            weapon_speed: 600.0,
        },
        // QuadCannonGun ground — PrimaryDamage 10, Range 150, Delay 100ms → 3 frames.
        SeedWeapon {
            name: QUAD_CANNON_GUN,
            primary_damage: 10.0,
            attack_range: 150.0,
            delay_frames: 3,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // QuadCannonGunUpgradeOne — dmg 8, Delay 50ms → 2 frames.
        SeedWeapon {
            name: QUAD_CANNON_GUN_UPGRADE_ONE,
            primary_damage: 8.0,
            attack_range: 150.0,
            delay_frames: 2,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // QuadCannonGunUpgradeTwo — dmg 8, Delay 25ms → 1 frame.
        SeedWeapon {
            name: QUAD_CANNON_GUN_UPGRADE_TWO,
            primary_damage: 8.0,
            attack_range: 150.0,
            delay_frames: 1,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // SCUDLauncherGunExplosive — PrimaryDamage 300, Range 350, clip 1,
        // ClipReload 10000ms → 300 frames. Area residual via host.
        SeedWeapon {
            name: SCUD_GUN_EXPLOSIVE,
            primary_damage: 300.0,
            attack_range: 350.0,
            delay_frames: 300,
            clip_size: 1,
            weapon_speed: 200.0,
        },
        // SCUDLauncherGunToxin — PrimaryDamage 200, Range 350, toxin residual.
        SeedWeapon {
            name: SCUD_GUN_TOXIN,
            primary_damage: 200.0,
            attack_range: 350.0,
            delay_frames: 300,
            clip_size: 1,
            weapon_speed: 200.0,
        },
        // SCUDLauncherGunAnthrax — same blast residual as toxin; upgraded field flag.
        SeedWeapon {
            name: SCUD_GUN_ANTHRAX,
            primary_damage: 200.0,
            attack_range: 350.0,
            delay_frames: 300,
            clip_size: 1,
            weapon_speed: 200.0,
        },
        // TechnicalMachineGunWeapon — dmg 10, range 150, Delay 200ms → 6 frames.
        SeedWeapon {
            name: TECHNICAL_MACHINE_GUN,
            primary_damage: 10.0,
            attack_range: 150.0,
            delay_frames: 6,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // TechnicalCannonWeapon — dmg 45, range 150, Delay 1000ms → 30 frames.
        SeedWeapon {
            name: TECHNICAL_CANNON,
            primary_damage: 45.0,
            attack_range: 150.0,
            delay_frames: 30,
            clip_size: 0,
            weapon_speed: 300.0,
        },
        // TechnicalRPGWeapon — dmg 50, range 150, Delay 1000ms → 30 frames.
        SeedWeapon {
            name: TECHNICAL_RPG,
            primary_damage: 50.0,
            attack_range: 150.0,
            delay_frames: 30,
            clip_size: 0,
            weapon_speed: 200.0,
        },
        // ToxinTruckGun — poison stream dmg 10, range 100, Delay 40ms → 2 frames.
        SeedWeapon {
            name: TOXIN_TRUCK_GUN,
            primary_damage: 10.0,
            attack_range: 100.0,
            delay_frames: 2,
            clip_size: 30,
            weapon_speed: 600.0,
        },
        // ToxinTruckGunUpgraded — anthrax stream dmg 12.5.
        SeedWeapon {
            name: TOXIN_TRUCK_GUN_UPGRADED,
            primary_damage: 12.5,
            attack_range: 100.0,
            delay_frames: 2,
            clip_size: 30,
            weapon_speed: 600.0,
        },
        // ToxinTruckSprayer — contaminate residual (retail PrimaryDamage 0; host
        // store needs >0 to bind — spray area dmg is SecondaryDamage 2 via residual).
        SeedWeapon {
            name: TOXIN_TRUCK_SPRAYER,
            primary_damage: 0.001,
            attack_range: 15.0,
            delay_frames: 6,
            clip_size: 0,
            weapon_speed: 600.0,
        },
        // ToxinTruckSprayerUpgraded — anthrax spray residual.
        SeedWeapon {
            name: TOXIN_TRUCK_SPRAYER_UPGRADED,
            primary_damage: 0.001,
            attack_range: 15.0,
            delay_frames: 6,
            clip_size: 0,
            weapon_speed: 600.0,
        },
        // MarauderTankGun — dmg 60, range 170, Delay 2000ms → 60 frames.
        SeedWeapon {
            name: MARAUDER_TANK_GUN,
            primary_damage: 60.0,
            attack_range: 170.0,
            delay_frames: 60,
            clip_size: 0,
            weapon_speed: 300.0,
        },
        // BattleMasterTankGun — dmg 60, range 150, Delay 2000ms → 60 frames.
        // UraniumShells PLAYER_UPGRADE DAMAGE 125% applied at host residual fire time.
        SeedWeapon {
            name: BATTLE_MASTER_TANK_GUN,
            primary_damage: 60.0,
            attack_range: 150.0,
            delay_frames: 60,
            clip_size: 0,
            weapon_speed: 400.0,
        },
        // ScorpionTankGun — dmg 20, range 150, Delay 1000ms → 30 frames.
        SeedWeapon {
            name: SCORPION_TANK_GUN,
            primary_damage: 20.0,
            attack_range: 150.0,
            delay_frames: 30,
            clip_size: 0,
            weapon_speed: 400.0,
        },
        // ScorpionTankGunPlusOne — salvage dmg 25.
        SeedWeapon {
            name: SCORPION_TANK_GUN_PLUS_ONE,
            primary_damage: 25.0,
            attack_range: 150.0,
            delay_frames: 30,
            clip_size: 0,
            weapon_speed: 400.0,
        },
        // ScorpionMissileWeapon — dmg 100, range 150, ClipReload 15000ms → 450 frames.
        SeedWeapon {
            name: SCORPION_MISSILE_WEAPON,
            primary_damage: 100.0,
            attack_range: 150.0,
            delay_frames: 450,
            clip_size: 1,
            weapon_speed: 600.0,
        },
        // TomahawkMissileWeapon — dmg 150, range 350, ClipReload 7000ms → 210 frames.
        SeedWeapon {
            name: TOMAHAWK_MISSILE_WEAPON,
            primary_damage: 150.0,
            attack_range: 350.0,
            delay_frames: 210,
            clip_size: 1,
            weapon_speed: 200.0,
        },
        // OverlordTankGun — dmg 80, range 175, ClipReload 2000ms → 60 frames, clip 2.
        SeedWeapon {
            name: OVERLORD_TANK_GUN,
            primary_damage: 80.0,
            attack_range: 175.0,
            delay_frames: 60,
            clip_size: 2,
            weapon_speed: 300.0,
        },
        // GLAJarmenKellRifle — dmg 180, range 225, Delay 1000ms → 30 frames.
        SeedWeapon {
            name: JARMEN_KELL_RIFLE,
            primary_damage: 180.0,
            attack_range: 225.0,
            delay_frames: 30,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // MarauderTankGunUpgradeOne — same dmg, Delay 1500ms → 45 frames.
        SeedWeapon {
            name: MARAUDER_TANK_GUN_UPGRADE_ONE,
            primary_damage: 60.0,
            attack_range: 170.0,
            delay_frames: 45,
            clip_size: 0,
            weapon_speed: 400.0,
        },
        // MarauderTankGunUpgradeTwo — same dmg, Delay 750ms → 23 frames, clip 2.
        SeedWeapon {
            name: MARAUDER_TANK_GUN_UPGRADE_TWO,
            primary_damage: 60.0,
            attack_range: 170.0,
            delay_frames: 23,
            clip_size: 2,
            weapon_speed: 500.0,
        },
        // GLARebelBikerMachineGun — dmg 8, range 150, Delay 100ms → 3 frames, clip 6.
        SeedWeapon {
            name: REBEL_BIKER_MG,
            primary_damage: 8.0,
            attack_range: 150.0,
            delay_frames: 3,
            clip_size: 6,
            weapon_speed: 999_999.0,
        },
        // TunnelDefenderBikerRocketWeapon — dmg 40, range 175, Delay 1000ms → 30 frames.
        SeedWeapon {
            name: TUNNEL_DEFENDER_BIKER_ROCKET,
            primary_damage: 40.0,
            attack_range: 175.0,
            delay_frames: 30,
            clip_size: 0,
            weapon_speed: 600.0,
        },
        // GLABikerKellSniperRifle — dmg 180, range 225, Delay 750ms → 23 frames.
        SeedWeapon {
            name: BIKER_KELL_SNIPER,
            primary_damage: 180.0,
            attack_range: 225.0,
            delay_frames: 23,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // TerroristSuicideWeapon residual — host binds as short-range suicide flag.
        SeedWeapon {
            name: TERRORIST_SUICIDE_WEAPON,
            primary_damage: 500.0,
            attack_range: 5.0,
            delay_frames: 1,
            clip_size: 1,
            weapon_speed: 999_999.0,
        },
        // SuicideDynamitePack residual — FireWeaponWhenDead for infantry Terrorist.
        SeedWeapon {
            name: SUICIDE_DYNAMITE_PACK,
            primary_damage: 500.0,
            attack_range: 5.0,
            delay_frames: 1,
            clip_size: 1,
            weapon_speed: 999_999.0,
        },
        // MissileDefenderMissileWeapon — dmg 40, range 175, Delay 1000ms → 30 frames.
        SeedWeapon {
            name: MISSILE_DEFENDER_MISSILE_WEAPON,
            primary_damage: 40.0,
            attack_range: 175.0,
            delay_frames: 30,
            clip_size: 0,
            weapon_speed: 600.0,
        },
        // MissileDefenderLaserGuidedMissileWeapon — dmg 40, range 300, Delay 500ms → 15 frames.
        SeedWeapon {
            name: MISSILE_DEFENDER_LASER_GUIDED_WEAPON,
            primary_damage: 40.0,
            attack_range: 300.0,
            delay_frames: 15,
            clip_size: 0,
            weapon_speed: 600.0,
        },
        // DragonTankFlameWeapon — dmg 10, range 75, Delay 40ms → 2 frames, splash residual.
        SeedWeapon {
            name: DRAGON_TANK_FLAME_WEAPON,
            primary_damage: 10.0,
            attack_range: 75.0,
            delay_frames: 2,
            clip_size: 30,
            weapon_speed: 600.0,
        },
        // DragonTankFlameWeaponUpgraded — BlackNapalm dmg 12.5.
        SeedWeapon {
            name: DRAGON_TANK_FLAME_WEAPON_UPGRADED,
            primary_damage: 12.5,
            attack_range: 75.0,
            delay_frames: 2,
            clip_size: 0,
            weapon_speed: 600.0,
        },
        // GattlingTankGun — dmg 15, range 150, Delay 400ms → 12 frames.
        SeedWeapon {
            name: GATTLING_TANK_GUN,
            primary_damage: 15.0,
            attack_range: 150.0,
            delay_frames: 12,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // Infa_MiniGunnerGun — dmg 10, range 125, Delay 500ms → 15 frames.
        SeedWeapon {
            name: MINIGUNNER_GUN,
            primary_damage: 10.0,
            attack_range: 125.0,
            delay_frames: 15,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // TunnelDefenderRocketWeapon — dmg 40, range 175, min 5, Delay 1000ms → 30 frames.
        SeedWeapon {
            name: TUNNEL_DEFENDER_ROCKET_WEAPON,
            primary_damage: 40.0,
            attack_range: 175.0,
            delay_frames: 30,
            clip_size: 0,
            weapon_speed: 600.0,
        },
        // DozerMineDisarmingWeapon — AttackRange 5, PreAttackDelay 1200ms → 36f,
        // ClipReloadTime 4000ms → 120f. DAMAGE_DISARM safe-clear residual
        // resolves via host_weapon_is_disarm_damage name match.
        SeedWeapon {
            name: DOZER_MINE_DISARMING_WEAPON,
            primary_damage: 1.0,
            attack_range: DOZER_MINE_CLEAR_RANGE,
            delay_frames: WORKER_MINE_CLEAR_DELAY_FRAMES as i32,
            clip_size: 1,
            weapon_speed: 999_999.0,
        },
        // WorkerMineDisarmingWeapon — AttackRange 5, PreAttackDelay 1000ms → 30f,
        // DelayBetweenShots 1000ms → 30f. DAMAGE_DISARM safe-clear residual.
        SeedWeapon {
            name: WORKER_MINE_DISARMING_WEAPON,
            primary_damage: 1.0,
            attack_range: DOZER_MINE_CLEAR_RANGE,
            delay_frames: WORKER_MINE_CLEAR_DELAY_FRAMES as i32,
            clip_size: 1,
            weapon_speed: 999_999.0,
        },
    ];

    let mut added = 0usize;
    for seed in seeds {
        if store_has(seed.name) {
            continue;
        }
        let mut t = WeaponTemplate::new(seed.name.to_string());
        t.primary_damage = seed.primary_damage;
        t.attack_range = seed.attack_range;
        t.min_delay_between_shots = seed.delay_frames;
        t.max_delay_between_shots = seed.delay_frames;
        t.clip_size = seed.clip_size;
        t.weapon_speed = seed.weapon_speed;
        t.damage_type = seed_damage_type_for(seed.name, seed.weapon_speed);
        t.death_type = seed_death_type_for(seed.name, t.damage_type);
        t.allow_attack_garrisoned_bldgs = seed.name == DRAGON_TANK_FLAME_WEAPON
            || seed.name == DRAGON_TANK_FLAME_WEAPON_UPGRADED
            || seed.name == RANGER_SECONDARY_WEAPON
            || seed.name == MICROWAVE_BUILDING_CLEARER_WEAPON;
        {
            let fs = seed_fire_sound_for(seed.name);
            if !fs.is_empty() {
                t.fire_sound = gamelogic::weapon::AudioEventRts::new(fs);
            }
            let pname = seed_projectile_name_for(seed.name);
            if !pname.is_empty() {
                t.projectile_name = pname;
            }
        }
        t.anti_mask.insert(WeaponAntiMask::GROUND);
        // Min-range residual for long-range GLA artillery / rockets.
        if seed.name == BUGGY_ROCKET_WEAPON || seed.name == BUGGY_ROCKET_WEAPON_UPGRADED {
            t.minimum_attack_range = 50.0;
        }
        if seed.name == SCUD_GUN_EXPLOSIVE
            || seed.name == SCUD_GUN_TOXIN
            || seed.name == SCUD_GUN_ANTHRAX
        {
            t.minimum_attack_range = 200.0;
            t.pre_attack_delay = 15; // 500ms @ 30 FPS residual
        }
        if seed.name == TECHNICAL_RPG {
            t.minimum_attack_range = 5.0;
        }
        if seed.name == TECHNICAL_CANNON {
            t.primary_damage_radius = 25.0;
        }
        if seed.name == MARAUDER_TANK_GUN
            || seed.name == MARAUDER_TANK_GUN_UPGRADE_ONE
            || seed.name == MARAUDER_TANK_GUN_UPGRADE_TWO
            || seed.name == BATTLE_MASTER_TANK_GUN
            || seed.name == SCORPION_TANK_GUN
            || seed.name == SCORPION_TANK_GUN_PLUS_ONE
        {
            t.primary_damage_radius = 5.0;
        }
        if seed.name == SCORPION_MISSILE_WEAPON {
            t.primary_damage_radius = 5.0;
            t.secondary_damage = 80.0;
            t.secondary_damage_radius = 25.0;
            t.minimum_attack_range = 40.0;
        }
        if seed.name == TOMAHAWK_MISSILE_WEAPON {
            t.primary_damage_radius = 10.0;
            t.secondary_damage = 50.0;
            t.secondary_damage_radius = 25.0;
            t.minimum_attack_range = 100.0;
            t.pre_attack_delay = 8; // 250ms @ 30 FPS residual
        }
        if seed.name == DRAGON_TANK_FLAME_WEAPON || seed.name == DRAGON_TANK_FLAME_WEAPON_UPGRADED {
            t.primary_damage_radius = 5.0;
            t.secondary_damage = if seed.name == DRAGON_TANK_FLAME_WEAPON_UPGRADED {
                1.25
            } else {
                1.0
            };
            t.secondary_damage_radius = 10.0;
        }
        if seed.name == TUNNEL_DEFENDER_BIKER_ROCKET {
            t.minimum_attack_range = 5.0;
            t.primary_damage_radius = 5.0;
            t.anti_mask.insert(WeaponAntiMask::AIRBORNE_VEHICLE);
            t.anti_mask.insert(WeaponAntiMask::AIRBORNE_INFANTRY);
        }
        if seed.name == TUNNEL_DEFENDER_ROCKET_WEAPON {
            t.minimum_attack_range = 5.0;
            t.primary_damage_radius = 5.0;
            t.anti_mask.insert(WeaponAntiMask::AIRBORNE_VEHICLE);
            t.anti_mask.insert(WeaponAntiMask::AIRBORNE_INFANTRY);
        }
        // Mine-clearing detail weapons: MINE + GROUND anti residual so the
        // FIRE_WEAPON ground-position carve-out (weapons.rs) and the mine
        // victim lookup both pass. PreAttackDelay / ClipReloadTime retail
        // residual (host_mines.rs constants).
        if seed.name == DOZER_MINE_DISARMING_WEAPON {
            t.pre_attack_delay = DOZER_MINE_CLEAR_PRE_ATTACK_FRAMES as i32;
            t.clip_reload_time = DOZER_MINE_CLEAR_CLIP_RELOAD_FRAMES as i32;
            t.anti_mask.insert(WeaponAntiMask::MINE);
        }
        if seed.name == WORKER_MINE_DISARMING_WEAPON {
            t.pre_attack_delay = WORKER_MINE_CLEAR_PRE_ATTACK_FRAMES as i32;
            // Retail Worker ClipReloadTime 4000ms → 120f — same as Dozer
            // (host_mines.rs has no separate worker clip-reload constant).
            t.clip_reload_time = DOZER_MINE_CLEAR_CLIP_RELOAD_FRAMES as i32;
            t.anti_mask.insert(WeaponAntiMask::MINE);
        }
        // Avenger designator can paint air + ground residual.
        if seed.name == AVENGER_TARGET_DESIGNATOR {
            t.anti_mask.insert(WeaponAntiMask::AIRBORNE_VEHICLE);
            t.anti_mask.insert(WeaponAntiMask::AIRBORNE_INFANTRY);
        }
        // Avenger air laser + Humvee air TOW: airborne only residual.
        if seed.name == AVENGER_AIR_LASER || seed.name == HUMVEE_MISSILE_WEAPON_AIR {
            t.anti_mask = WeaponAntiMask::new(0);
            t.anti_mask.insert(WeaponAntiMask::AIRBORNE_VEHICLE);
            t.anti_mask.insert(WeaponAntiMask::AIRBORNE_INFANTRY);
        }
        // Base-defense AA secondaries: airborne only residual.
        if seed.name == PATRIOT_SECONDARY_WEAPON
            || seed.name == LAZR_PATRIOT_SECONDARY_WEAPON
            || seed.name == SUPW_PATRIOT_SECONDARY_WEAPON
            || seed.name == STINGER_SECONDARY_WEAPON
            || seed.name == GATTLING_BUILDING_SECONDARY_WEAPON
        {
            t.anti_mask = WeaponAntiMask::new(0);
            t.anti_mask.insert(WeaponAntiMask::AIRBORNE_VEHICLE);
            t.anti_mask.insert(WeaponAntiMask::AIRBORNE_INFANTRY);
        }
        // Stinger / Patriot primary splash residual.
        if seed.name == STINGER_PRIMARY_WEAPON
            || seed.name == PATRIOT_PRIMARY_WEAPON
            || seed.name == PATRIOT_SECONDARY_WEAPON
            || seed.name == LAZR_PATRIOT_PRIMARY_WEAPON
            || seed.name == LAZR_PATRIOT_SECONDARY_WEAPON
            || seed.name == STINGER_SECONDARY_WEAPON
        {
            t.primary_damage_radius = if seed.name == LAZR_PATRIOT_PRIMARY_WEAPON
                || seed.name == LAZR_PATRIOT_SECONDARY_WEAPON
            {
                3.0
            } else {
                5.0
            };
        }
        // C++ Weapon.ini RequestAssistRange 200 — leftover store field live fire reads.
        if seed.name == PATRIOT_PRIMARY_WEAPON
            || seed.name == PATRIOT_SECONDARY_WEAPON
            || seed.name == LAZR_PATRIOT_PRIMARY_WEAPON
            || seed.name == LAZR_PATRIOT_SECONDARY_WEAPON
            || seed.name == SUPW_PATRIOT_PRIMARY_WEAPON
            || seed.name == SUPW_PATRIOT_SECONDARY_WEAPON
        {
            t.request_assist_range = 200.0;
        }
        if seed.name == STINGER_SECONDARY_WEAPON {
            t.primary_damage_radius = 10.0;
        }
        if seed.name == RANGER_PRIMARY_WEAPON {
            seed_ranger_drone_spotting_extra(&mut t);
        }

        match with_weapon_store_mut(|store| {
            store.add_weapon_template(t);
        }) {
            Ok(()) => {
                log::debug!("Host WeaponStore: seeded weapon {}", seed.name);
                added += 1;
            }
            Err(e) => {
                log::warn!("Host WeaponStore: failed to seed {}: {e}", seed.name);
            }
        }
    }

    // Quad Cannon AA secondaries: airborne only (AntiGround=No residual).
    for (name, delay) in [
        (QUAD_CANNON_GUN_AIR, 3i32),
        (QUAD_CANNON_GUN_UPGRADE_ONE_AIR, 2),
        (QUAD_CANNON_GUN_UPGRADE_TWO_AIR, 1),
    ] {
        if store_has(name) {
            continue;
        }
        let mut t = WeaponTemplate::new(name.to_string());
        t.primary_damage = 5.0;
        t.attack_range = 350.0;
        t.min_delay_between_shots = delay;
        t.max_delay_between_shots = delay;
        t.clip_size = 0;
        t.weapon_speed = 999_999.0;
        // Air only — no GROUND mask so can_target_ground residual is false.
        t.anti_mask = WeaponAntiMask::new(0);
        t.anti_mask.insert(WeaponAntiMask::AIRBORNE_VEHICLE);
        t.anti_mask.insert(WeaponAntiMask::AIRBORNE_INFANTRY);
        match with_weapon_store_mut(|store| {
            store.add_weapon_template(t);
        }) {
            Ok(()) => {
                log::debug!("Host WeaponStore: seeded AA weapon {}", name);
                added += 1;
            }
            Err(e) => {
                log::warn!("Host WeaponStore: failed to seed {}: {e}", name);
            }
        }
    }

    // Gattling Tank AA secondary: airborne only residual.
    if !store_has(GATTLING_TANK_GUN_AIR) {
        let mut t = WeaponTemplate::new(GATTLING_TANK_GUN_AIR.to_string());
        t.primary_damage = 12.0;
        t.attack_range = 350.0;
        t.min_delay_between_shots = 12;
        t.max_delay_between_shots = 12;
        t.clip_size = 0;
        t.weapon_speed = 999_999.0;
        t.anti_mask = WeaponAntiMask::new(0);
        t.anti_mask.insert(WeaponAntiMask::AIRBORNE_VEHICLE);
        t.anti_mask.insert(WeaponAntiMask::AIRBORNE_INFANTRY);
        match with_weapon_store_mut(|store| {
            store.add_weapon_template(t);
        }) {
            Ok(()) => {
                log::debug!(
                    "Host WeaponStore: seeded AA weapon {}",
                    GATTLING_TANK_GUN_AIR
                );
                added += 1;
            }
            Err(e) => {
                log::warn!(
                    "Host WeaponStore: failed to seed {}: {e}",
                    GATTLING_TANK_GUN_AIR
                );
            }
        }
    }

    // MiniGunner AA secondary: airborne only residual.
    if !store_has(MINIGUNNER_GUN_AIR) {
        let mut t = WeaponTemplate::new(MINIGUNNER_GUN_AIR.to_string());
        t.primary_damage = 10.0;
        t.attack_range = 350.0;
        t.min_delay_between_shots = 15;
        t.max_delay_between_shots = 15;
        t.clip_size = 0;
        t.weapon_speed = 999_999.0;
        t.anti_mask = WeaponAntiMask::new(0);
        t.anti_mask.insert(WeaponAntiMask::AIRBORNE_VEHICLE);
        t.anti_mask.insert(WeaponAntiMask::AIRBORNE_INFANTRY);
        match with_weapon_store_mut(|store| {
            store.add_weapon_template(t);
        }) {
            Ok(()) => {
                log::debug!("Host WeaponStore: seeded AA weapon {}", MINIGUNNER_GUN_AIR);
                added += 1;
            }
            Err(e) => {
                log::warn!(
                    "Host WeaponStore: failed to seed {}: {e}",
                    MINIGUNNER_GUN_AIR
                );
            }
        }
    }

    if added > 0 {
        log::info!(
            "Host WeaponStore: seeded {} known golden-unit weapons (INI data unavailable or incomplete)",
            added
        );
    }
    added
}

pub(super) struct SeedWeapon {
    pub(super) name: &'static str,
    pub(super) primary_damage: f32,
    pub(super) attack_range: f32,
    pub(super) delay_frames: i32,
    pub(super) clip_size: i32,
    pub(super) weapon_speed: f32,
}

/// Test helper: force re-bootstrap attempt (does not clear existing templates).
#[cfg(test)]
pub fn reset_bootstrap_attempt_flag_for_tests() {
    BOOTSTRAP_ATTEMPTED.store(false, Ordering::Relaxed);
    SEED_COMPLETE.store(false, Ordering::Relaxed);
}
