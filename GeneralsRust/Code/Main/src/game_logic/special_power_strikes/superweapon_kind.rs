//! HostSuperweaponKind mapping, damage/audio tables, and multi-strike planners.
use super::types::*;
use super::*;
/// Leftover `special_power_effects.rs` NapalmStrike fire table (not DaisyCutter 2000 EXPLOSION).
pub const NAPALM_STRIKE_PRIMARY_DAMAGE: f32 = 150.0;
/// Leftover NapalmStrike fire PrimaryDamageRadius.
pub const NAPALM_STRIKE_PRIMARY_RADIUS: f32 = 60.0;
/// Leftover ocl_power.rs SuperweaponNapalmStrike radius / RadiusCursorRadius.
pub const NAPALM_STRIKE_OUTER_RADIUS: f32 = 100.0;
/// C++ OCLSpecialPower findOCL own payload residual.
pub const NAPALM_STRIKE_OCL: &str = "SUPERWEAPON_NapalmStrike";
/// OCL cargo-plane approach residual (same family as other edge-spawn OCLs).
pub const NAPALM_STRIKE_IMPACT_DELAY_FRAMES: u32 = 90;

/// Host-supported superweapon strike kinds for this residual path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostSuperweaponKind {
    /// USA Daisy Cutter / Fuel Air Bomb / MOAB family.
    DaisyCutter,
    /// USA A-10 Thunderbolt missile strike.
    A10Strike,
    /// GLA SCUD Storm.
    ScudStorm,
    /// China/USA Particle Uplink Cannon continuous beam residual host path.
    ParticleCannon,
    /// China Nuclear Missile / NeutronMissile residual host path.
    NuclearMissile,
    /// GLA Anthrax Bomb residual host path (plane drop + toxin field).
    AnthraxBomb,
    /// USA Spectre Gunship residual host path (delayed orbit + damage ticks).
    SpectreGunship,
    /// Carpet Bomb residual host path (delayed line multi-strike damage).
    CarpetBomb,
    /// China Artillery Barrage residual host path (delayed multi-shell scatter).
    ArtilleryBarrage,
    /// USA Superweapon General Cruise Missile residual host path
    /// (delayed loft + MOABDetonationWeapon area damage).
    CruiseMissile,
    /// China Napalm Strike residual — own OCL / fire table, not DaisyCutter FAB.
    NapalmStrike,
}

impl HostSuperweaponKind {
    /// Map a command-system power type to a host residual strike, if supported.
    pub fn from_command_power(power: &SpecialPowerType) -> Option<Self> {
        match power {
            SpecialPowerType::DaisyCutter
            | SpecialPowerType::FuelAirBomb
            | SpecialPowerType::AirForceDaisyCutter => Some(HostSuperweaponKind::DaisyCutter),
            SpecialPowerType::NapalmStrike => Some(HostSuperweaponKind::NapalmStrike),
            SpecialPowerType::Airstrike | SpecialPowerType::AirForceAirstrike => {
                Some(HostSuperweaponKind::A10Strike)
            }
            SpecialPowerType::ScudStorm => Some(HostSuperweaponKind::ScudStorm),
            SpecialPowerType::ParticleCannon
            | SpecialPowerType::SuperweaponParticleCannon
            | SpecialPowerType::LaserCannon => Some(HostSuperweaponKind::ParticleCannon),
            SpecialPowerType::NuclearMissile
            | SpecialPowerType::BlackMarketNuke
            | SpecialPowerType::DetonateDirtyNuke
            | SpecialPowerType::NukeNeutronMissile
            | SpecialPowerType::SuperweaponNeutronMissile
            | SpecialPowerType::BaikonurRocket => Some(HostSuperweaponKind::NuclearMissile),
            SpecialPowerType::AnthraxBomb => Some(HostSuperweaponKind::AnthraxBomb),
            SpecialPowerType::SpectreGunship | SpecialPowerType::AirForceSpectreGunship => {
                Some(HostSuperweaponKind::SpectreGunship)
            }
            SpecialPowerType::CarpetBomb
            | SpecialPowerType::EarlyChinaCarpetBomb
            | SpecialPowerType::AirForceCarpetBomb
            | SpecialPowerType::NukeChinaCarpetBomb => Some(HostSuperweaponKind::CarpetBomb),
            SpecialPowerType::Artillery => Some(HostSuperweaponKind::ArtilleryBarrage),
            SpecialPowerType::CruiseMissile => Some(HostSuperweaponKind::CruiseMissile),
            _ => None,
        }
    }

    /// Representative command power used for ScriptEngine COMPLETED notify.
    pub fn command_power_for_notify(self) -> Option<SpecialPowerType> {
        Some(match self {
            HostSuperweaponKind::DaisyCutter => SpecialPowerType::DaisyCutter,
            HostSuperweaponKind::A10Strike => SpecialPowerType::Airstrike,
            HostSuperweaponKind::ScudStorm => SpecialPowerType::ScudStorm,
            HostSuperweaponKind::ParticleCannon => SpecialPowerType::ParticleCannon,
            HostSuperweaponKind::NuclearMissile => SpecialPowerType::NuclearMissile,
            HostSuperweaponKind::AnthraxBomb => SpecialPowerType::AnthraxBomb,
            HostSuperweaponKind::SpectreGunship => SpecialPowerType::SpectreGunship,
            HostSuperweaponKind::CarpetBomb => SpecialPowerType::CarpetBomb,
            HostSuperweaponKind::ArtilleryBarrage => SpecialPowerType::Artillery,
            HostSuperweaponKind::CruiseMissile => SpecialPowerType::CruiseMissile,
            HostSuperweaponKind::NapalmStrike => SpecialPowerType::NapalmStrike,
        })
    }

    /// Human-readable label for logs / honesty reports.
    pub fn label(self) -> &'static str {
        match self {
            HostSuperweaponKind::DaisyCutter => "DaisyCutter",
            HostSuperweaponKind::A10Strike => "A10Strike",
            HostSuperweaponKind::ScudStorm => "ScudStorm",
            HostSuperweaponKind::ParticleCannon => "ParticleCannon",
            HostSuperweaponKind::NuclearMissile => "NuclearMissile",
            HostSuperweaponKind::AnthraxBomb => "AnthraxBomb",
            HostSuperweaponKind::SpectreGunship => "SpectreGunship",
            HostSuperweaponKind::CarpetBomb => "CarpetBomb",
            HostSuperweaponKind::ArtilleryBarrage => "ArtilleryBarrage",
            HostSuperweaponKind::CruiseMissile => "CruiseMissile",
            HostSuperweaponKind::NapalmStrike => "NapalmStrike",
        }
    }

    /// C++ `SpecialPowerTemplate::getViewObjectRange` residual (retail 250).
    pub fn view_object_range(self) -> f32 {
        250.0
    }

    /// C++ `SpecialPowerTemplate::getViewObjectDuration` frames residual.
    /// Retail Superweapon INI is 30s (900f) or 40s (1200f).
    pub fn view_object_duration_frames(self) -> u32 {
        match self {
            HostSuperweaponKind::NuclearMissile
            | HostSuperweaponKind::CarpetBomb
            | HostSuperweaponKind::ParticleCannon => 1_200,
            _ => 900,
        }
    }

    /// Impact delay in logic frames before area damage applies.
    pub fn impact_delay_frames(self) -> u32 {
        match self {
            // FuelAirBombPower residual: impact_delay 3.0s @ 30 FPS.
            HostSuperweaponKind::DaisyCutter => DAISY_CUTTER_IMPACT_DELAY_FRAMES,
            // A-10 flight/approach residual (shorter than full aircraft OCL).
            HostSuperweaponKind::A10Strike => A10_STRIKE_IMPACT_DELAY_FRAMES,
            // SCUD PreAttackDelay residual (first missile); multi-missile stagger follows.
            HostSuperweaponKind::ScudStorm => SCUD_STORM_PRE_ATTACK_FRAMES,
            // C++ orbitalBirthFrame = startAttack + BeamTravelTime (2500ms → 75f).
            // First pulse is on orbital birth (`m_nextDamagePulseFrame = now`).
            // Leftover ParticleUplinkCannonUpdate already matches; live residual
            // spawn/first tick is impact_frame (see HostParticleBeamField).
            HostSuperweaponKind::ParticleCannon => PARTICLE_BEAM_TRAVEL_FRAMES,
            // NeutronMissile residual flight/approach (fail-closed vs full
            // NeutronMissileUpdate loft + SpecialSpeedTime path).
            HostSuperweaponKind::NuclearMissile => 180,
            // GLA Jet cargo plane drop residual (same family as DaisyCutter).
            HostSuperweaponKind::AnthraxBomb => 90,
            // Spectre insertion residual (fail-closed vs full edge spawn +
            // transit locomotor + GUNSHIP_STATUS_INSERTING approach).
            HostSuperweaponKind::SpectreGunship => 90,
            // Carpet bomber approach residual (fail-closed vs full edge spawn +
            // DeliverPayload transit + staggered DropDelay).
            HostSuperweaponKind::CarpetBomb => CARPET_BOMB_IMPACT_DELAY_FRAMES,
            // Retail DelayDeliveryMax = 3000 ms residual (artillerist reaction).
            HostSuperweaponKind::ArtilleryBarrage => ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES,
            // Cruise loft residual (NeutronMissileUpdate family; doors deferred).
            HostSuperweaponKind::CruiseMissile => CRUISE_MISSILE_IMPACT_DELAY_FRAMES,
            // China NapalmStrike OCL cargo-plane approach residual (own payload).
            HostSuperweaponKind::NapalmStrike => NAPALM_STRIKE_IMPACT_DELAY_FRAMES,
        }
    }

    /// Retail SpecialPower.ini `ReloadTime` residual (msec) for the baseline host kind.
    ///
    /// Faction/science variants (AirF Spectre 180s, SupW PUC 180s, AirF Carpet 240s,
    /// Nuke China Carpet 180s, …) stay on dedicated constants; this table freezes the
    /// primary `HostSuperweaponKind` SpecialPower residual used by host residual queues.
    pub fn reload_ms(self) -> u32 {
        match self {
            HostSuperweaponKind::DaisyCutter => DAISY_CUTTER_RELOAD_MS,
            HostSuperweaponKind::A10Strike => A10_STRIKE_RELOAD_MS,
            HostSuperweaponKind::ScudStorm => SCUD_STORM_RELOAD_MS,
            HostSuperweaponKind::ParticleCannon => PARTICLE_CANNON_RELOAD_MS,
            HostSuperweaponKind::NuclearMissile => NUCLEAR_MISSILE_RELOAD_MS,
            HostSuperweaponKind::AnthraxBomb => ANTHRAX_BOMB_RELOAD_MS,
            HostSuperweaponKind::SpectreGunship => SPECTRE_RELOAD_MS,
            HostSuperweaponKind::CarpetBomb => CARPET_BOMB_RELOAD_MS,
            HostSuperweaponKind::ArtilleryBarrage => ARTILLERY_BARRAGE_RELOAD_MS,
            HostSuperweaponKind::CruiseMissile => CRUISE_MISSILE_RELOAD_MS,
            HostSuperweaponKind::NapalmStrike => NAPALM_STRIKE_RELOAD_MS,
        }
    }

    /// ReloadTime frames residual (@ 30 FPS ceil) for the baseline host kind.
    pub fn reload_frames(self) -> u32 {
        duration_ms_to_logic_frames(self.reload_ms())
    }

    /// Max damage at epicenter (host residual values; retail weapon tables deferred).
    pub fn max_damage(self) -> f32 {
        match self {
            HostSuperweaponKind::DaisyCutter => DAISY_CUTTER_PRIMARY_DAMAGE,
            HostSuperweaponKind::A10Strike => A10_STRIKE_HOST_MAX_DAMAGE,
            // Retail ScudStormDamageWeapon PrimaryDamage (per missile).
            HostSuperweaponKind::ScudStorm => SCUD_STORM_PRIMARY_DAMAGE,
            // Continuous beam residual: no one-shot impact blast
            // (damage via HostParticleBeamField pulses — DamagePerSecond 400).
            HostSuperweaponKind::ParticleCannon => 0.0,
            // Retail NeutronMissileSlowDeath Blast6MaxDamage (scheduled residual).
            HostSuperweaponKind::NuclearMissile => 3500.0,
            // Retail AnthraxBombWeapon PrimaryDamage (impact blast only).
            HostSuperweaponKind::AnthraxBomb => 200.0,
            // Spectre has no one-shot impact blast; damage is orbit residual only.
            HostSuperweaponKind::SpectreGunship => 0.0,
            // Retail CarpetBombWeapon PrimaryDamage (per bomb epicenter).
            HostSuperweaponKind::CarpetBomb => CARPET_BOMB_DAMAGE,
            // Retail ArtilleryBarrageDamageWeapon PrimaryDamage (per shell).
            HostSuperweaponKind::ArtilleryBarrage => ARTILLERY_BARRAGE_DAMAGE,
            // Retail MOABDetonationWeapon PrimaryDamage (CruiseMissile death weapon).
            HostSuperweaponKind::CruiseMissile => CRUISE_MISSILE_DAMAGE,
            // Leftover special_power_effects NapalmStrike fire table (not FAB 2000).
            HostSuperweaponKind::NapalmStrike => NAPALM_STRIKE_PRIMARY_DAMAGE,
        }
    }

    /// Outer damage radius (matches SpecialPower.ini RadiusCursorRadius where known).
    pub fn damage_radius(self) -> f32 {
        match self {
            HostSuperweaponKind::DaisyCutter => DAISY_CUTTER_OUTER_RADIUS,
            HostSuperweaponKind::A10Strike => A10_STRIKE_HOST_RADIUS,
            // Retail ScudStormDamageWeapon SecondaryDamageRadius.
            HostSuperweaponKind::ScudStorm => SCUD_STORM_SECONDARY_RADIUS,
            // Residual beam damage radius (see PARTICLE_BEAM_RADIUS).
            HostSuperweaponKind::ParticleCannon => PARTICLE_BEAM_RADIUS,
            // Retail Blast6OuterRadius / DeliveryDecalRadius.
            HostSuperweaponKind::NuclearMissile => 210.0,
            // Retail AnthraxBombWeapon PrimaryDamageRadius.
            HostSuperweaponKind::AnthraxBomb => 100.0,
            // Retail AttackAreaRadius / RadiusCursorRadius.
            HostSuperweaponKind::SpectreGunship => SPECTRE_ORBIT_RADIUS,
            // Retail CarpetBombWeapon PrimaryDamageRadius (per bomb).
            HostSuperweaponKind::CarpetBomb => CARPET_BOMB_RADIUS,
            // Retail ArtilleryBarrageDamageWeapon PrimaryDamageRadius (per shell).
            HostSuperweaponKind::ArtilleryBarrage => ARTILLERY_BARRAGE_RADIUS,
            // Retail MOABDetonationWeapon PrimaryDamageRadius.
            HostSuperweaponKind::CruiseMissile => CRUISE_MISSILE_RADIUS,
            // Leftover ocl_power SuperweaponNapalmStrike radius / RadiusCursor.
            HostSuperweaponKind::NapalmStrike => NAPALM_STRIKE_OUTER_RADIUS,
        }
    }

    /// Inner radius with full damage (two-stage falloff).
    pub fn falloff_inner(self) -> f32 {
        match self {
            HostSuperweaponKind::DaisyCutter => DAISY_CUTTER_PRIMARY_RADIUS,
            HostSuperweaponKind::A10Strike => A10_STRIKE_HOST_INNER_RADIUS,
            // Retail ScudStormDamageWeapon PrimaryDamageRadius (full primary).
            HostSuperweaponKind::ScudStorm => SCUD_STORM_PRIMARY_RADIUS,
            // Continuous beam: no one-shot falloff (pulse damage is flat in radius).
            HostSuperweaponKind::ParticleCannon => 0.0,
            // Retail Blast6InnerRadius.
            HostSuperweaponKind::NuclearMissile => 60.0,
            // Flat primary blast (no secondary falloff in weapon table).
            HostSuperweaponKind::AnthraxBomb => 100.0,
            // No impact blast falloff (orbit residual handles damage).
            HostSuperweaponKind::SpectreGunship => 0.0,
            // Flat primary blast per bomb epicenter.
            HostSuperweaponKind::CarpetBomb => CARPET_BOMB_RADIUS,
            // Flat primary blast per shell epicenter.
            HostSuperweaponKind::ArtilleryBarrage => ARTILLERY_BARRAGE_RADIUS,
            // Residual two-stage falloff for MOAB primary blast.
            HostSuperweaponKind::CruiseMissile => CRUISE_MISSILE_FALLOFF_INNER,
            // Leftover NapalmStrike fire PrimaryDamageRadius (flat fire inside).
            HostSuperweaponKind::NapalmStrike => NAPALM_STRIKE_PRIMARY_RADIUS,
        }
    }

    /// Whether impact should spawn a residual radiation field.
    pub fn spawns_radiation(self) -> bool {
        matches!(self, HostSuperweaponKind::NuclearMissile)
    }

    /// Whether impact should spawn a residual toxin / anthrax / scud poison field.
    pub fn spawns_toxin_field(self) -> bool {
        matches!(
            self,
            HostSuperweaponKind::AnthraxBomb | HostSuperweaponKind::ScudStorm
        )
    }

    /// Whether toxin residual uses ScudStorm LargePoisonField stats (vs Anthrax bomb).
    pub fn spawns_scud_poison_field(self) -> bool {
        matches!(self, HostSuperweaponKind::ScudStorm)
    }

    /// Whether impact should spawn a residual Spectre orbit damage field.
    pub fn spawns_orbit_field(self) -> bool {
        matches!(self, HostSuperweaponKind::SpectreGunship)
    }

    /// Whether impact should spawn a residual Particle Uplink continuous beam field.
    pub fn spawns_beam_field(self) -> bool {
        matches!(self, HostSuperweaponKind::ParticleCannon)
    }

    /// Whether this kind applies multi-point line damage (CarpetBomb residual).
    pub fn is_line_multi_strike(self) -> bool {
        matches!(self, HostSuperweaponKind::CarpetBomb)
    }

    /// Whether this kind applies multi-shell scatter damage (ArtilleryBarrage residual).
    pub fn is_scatter_multi_strike(self) -> bool {
        matches!(self, HostSuperweaponKind::ArtilleryBarrage)
    }

    /// Whether this kind applies multi-missile ScatterTarget residual (ScudStorm).
    pub fn is_scud_multi_strike(self) -> bool {
        matches!(self, HostSuperweaponKind::ScudStorm)
    }

    /// Whether this kind uses multi-point epicenter damage at impact.
    pub fn is_multi_strike(self) -> bool {
        self.is_line_multi_strike() || self.is_scatter_multi_strike() || self.is_scud_multi_strike()
    }

    /// Whether retail `RadiusDamageAffects` includes ALLIES for the primary blast.
    ///
    /// Host residual previously excluded friendlies fail-closed. Wave 11 closes
    /// ally-hit residual for kinds whose Weapon.ini lists ALLIES.
    pub fn hits_allies(self) -> bool {
        matches!(
            self,
            HostSuperweaponKind::DaisyCutter
                | HostSuperweaponKind::A10Strike
                | HostSuperweaponKind::ScudStorm
                | HostSuperweaponKind::NuclearMissile
                | HostSuperweaponKind::AnthraxBomb
                | HostSuperweaponKind::CarpetBomb
                | HostSuperweaponKind::ArtilleryBarrage
                | HostSuperweaponKind::CruiseMissile
                | HostSuperweaponKind::NapalmStrike
        )
    }

    /// Whether impact also applies retail `MOABFlameWeapon` secondary residual
    /// (MOABGas SlowDeath MIDPOINT flame — tree-ignite / FLAME damage).
    pub fn spawns_moab_flame(self) -> bool {
        matches!(
            self,
            HostSuperweaponKind::DaisyCutter | HostSuperweaponKind::CruiseMissile
        )
    }

    /// Residual multi-point shell/bomb epicenters for multi-strike kinds.
    pub fn multi_strike_points(self, target: Vec3) -> Option<Vec<Vec3>> {
        self.multi_strike_points_with_tier(target, ArtilleryBarrageScienceTier::Level1)
    }

    /// Residual multi-point epicenters with ArtilleryBarrage science-tier FormationSize.
    pub fn multi_strike_points_with_tier(
        self,
        target: Vec3,
        artillery_tier: ArtilleryBarrageScienceTier,
    ) -> Option<Vec<Vec3>> {
        if self.is_line_multi_strike() {
            Some(carpet_bomb_points(target))
        } else if self.is_scatter_multi_strike() {
            Some(artillery_barrage_points_for_tier(target, artillery_tier))
        } else if self.is_scud_multi_strike() {
            Some(scud_storm_points(target))
        } else {
            None
        }
    }

    /// Audio event name queued on activation (host residual).
    ///
    /// Host residual uses special-power template labels for queue honesty.
    /// Retail Miles event names live in [`Self::retail_initiate_sound`] /
    /// [`Self::retail_initiate_at_location_sound`] (Wave 77 name tables).
    pub fn activate_audio(self) -> &'static str {
        match self {
            HostSuperweaponKind::DaisyCutter => "SuperweaponDaisyCutter",
            HostSuperweaponKind::A10Strike => "SuperweaponA10Strike",
            HostSuperweaponKind::ScudStorm => "SuperweaponScudStorm",
            HostSuperweaponKind::ParticleCannon => "SuperweaponParticleCannon",
            HostSuperweaponKind::NuclearMissile => "SuperweaponNuclearMissile",
            HostSuperweaponKind::AnthraxBomb => "SuperweaponAnthraxBomb",
            HostSuperweaponKind::SpectreGunship => "SuperweaponSpectreGunship",
            HostSuperweaponKind::CarpetBomb => "SuperweaponCarpetBomb",
            HostSuperweaponKind::ArtilleryBarrage => "SuperweaponArtilleryBarrage",
            HostSuperweaponKind::CruiseMissile => "SuperweaponCruiseMissile",
            HostSuperweaponKind::NapalmStrike => "SuperweaponNapalmStrike",
        }
    }

    /// Retail SpecialPower.ini `InitiateSound` residual (plays at source).
    ///
    /// Empty when retail omits the field or comments it out. Fail-closed: not
    /// full Miles event playback — name-table residual only (Wave 77).
    pub fn retail_initiate_sound(self) -> &'static str {
        match self {
            HostSuperweaponKind::ScudStorm => SCUD_STORM_INITIATE_SOUND,
            HostSuperweaponKind::ArtilleryBarrage => ARTILLERY_BARRAGE_INITIATE_SOUND,
            HostSuperweaponKind::CruiseMissile => CRUISE_MISSILE_INITIATE_SOUND,
            // Neutron InitiateSound is commented out in retail SpecialPower.ini.
            HostSuperweaponKind::NuclearMissile => NUCLEAR_MISSILE_INITIATE_SOUND,
            HostSuperweaponKind::DaisyCutter
            | HostSuperweaponKind::A10Strike
            | HostSuperweaponKind::ParticleCannon
            | HostSuperweaponKind::AnthraxBomb
            | HostSuperweaponKind::SpectreGunship
            | HostSuperweaponKind::CarpetBomb
            | HostSuperweaponKind::NapalmStrike => EMPTY_SPECIAL_POWER_INITIATE_SOUND,
        }
    }

    /// Retail SpecialPower.ini `InitiateAtLocationSound` residual (plays at target).
    ///
    /// Empty when retail omits the field. Fail-closed: name-table residual only.
    pub fn retail_initiate_at_location_sound(self) -> &'static str {
        match self {
            HostSuperweaponKind::NuclearMissile => NUCLEAR_MISSILE_INITIATE_AT_LOCATION_SOUND,
            HostSuperweaponKind::CruiseMissile => CRUISE_MISSILE_INITIATE_AT_LOCATION_SOUND,
            HostSuperweaponKind::ScudStorm => SCUD_STORM_INITIATE_AT_LOCATION_SOUND,
            HostSuperweaponKind::ArtilleryBarrage => ARTILLERY_BARRAGE_INITIATE_AT_LOCATION_SOUND,
            HostSuperweaponKind::DaisyCutter
            | HostSuperweaponKind::A10Strike
            | HostSuperweaponKind::ParticleCannon
            | HostSuperweaponKind::AnthraxBomb
            | HostSuperweaponKind::SpectreGunship
            | HostSuperweaponKind::CarpetBomb
            | HostSuperweaponKind::NapalmStrike => EMPTY_SPECIAL_POWER_INITIATE_SOUND,
        }
    }

    /// Audio event name queued on impact (host residual).
    pub fn impact_audio(self) -> &'static str {
        match self {
            HostSuperweaponKind::DaisyCutter => "DaisyCutterExplosion",
            // C++ no consolidated A10 cursor blast / invented A10StrikeImpact.
            HostSuperweaponKind::A10Strike => A10_STRIKE_IMPACT_AUDIO,
            HostSuperweaponKind::ScudStorm => "ScudStormImpact",
            // Beam contact residual (continuous pulses follow).
            HostSuperweaponKind::ParticleCannon => "ParticleCannonBeamStart",
            HostSuperweaponKind::NuclearMissile => "NuclearMissileImpact",
            HostSuperweaponKind::AnthraxBomb => "AnthraxBombImpact",
            // Orbit insertion complete residual (retail SpectreGunshipVoiceArrive).
            HostSuperweaponKind::SpectreGunship => "SpectreGunshipVoiceArrive",
            // Retail ExplosionCarpetBomb residual cue.
            HostSuperweaponKind::CarpetBomb => "ExplosionCarpetBomb",
            // Retail FX_ArtilleryBarrage residual cue.
            HostSuperweaponKind::ArtilleryBarrage => "FX_ArtilleryBarrage",
            // Retail WeaponFX_MOAB_Blast residual cue.
            HostSuperweaponKind::CruiseMissile => "CruiseMissileImpact",
            HostSuperweaponKind::NapalmStrike => "NapalmStrikeImpact",
        }
    }

    /// Authored blast weapon template (Weapon.ini) for this kind's one-shot.
    pub fn blast_weapon_name(self) -> Option<&'static str> {
        match self {
            HostSuperweaponKind::DaisyCutter => Some(DAISY_CUTTER_WEAPON_NAME),
            HostSuperweaponKind::A10Strike => Some(A10_PAYLOAD_WEAPON),
            HostSuperweaponKind::ScudStorm => Some(SCUD_STORM_MISSILE_DEATH_WEAPON_BASE),
            HostSuperweaponKind::ParticleCannon => None,
            HostSuperweaponKind::NuclearMissile => None,
            HostSuperweaponKind::AnthraxBomb => Some(ANTHRAX_BOMB_WEAPON_NAME),
            HostSuperweaponKind::SpectreGunship => None,
            HostSuperweaponKind::CarpetBomb => Some(CARPET_BOMB_WEAPON_NAME),
            HostSuperweaponKind::ArtilleryBarrage => Some(ARTILLERY_BARRAGE_WEAPON_NAME),
            HostSuperweaponKind::CruiseMissile => Some(CRUISE_MISSILE_DEATH_WEAPON),
            HostSuperweaponKind::NapalmStrike => None,
        }
    }

    /// C++ DamageInfo.m_damageType for the blast (Weapon.ini / SlowDeath).
    /// Live take_damage must use this so Armor.ini applies. Not UNRESISTABLE.
    pub fn authored_damage_type(self) -> crate::game_logic::combat::DamageType {
        use crate::game_logic::combat::DamageType;
        if let Some(name) = self.blast_weapon_name() {
            let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
            if crate::game_logic::thing::ThingTemplate::weapon_from_store(name).is_some() {
                return crate::game_logic::host_armor_residual::host_damage_type_for_weapon_name(
                    name,
                );
            }
        }
        match self {
            HostSuperweaponKind::ParticleCannon => DamageType::ParticleBeam,
            HostSuperweaponKind::NapalmStrike => DamageType::Fire,
            _ => DamageType::Explosive,
        }
    }

    /// C++ DamageInfo.m_deathType for the blast.
    pub fn authored_death_type(self) -> crate::game_logic::host_usa_pilot::HostDeathType {
        use crate::game_logic::host_usa_pilot::HostDeathType;
        if let Some(name) = self.blast_weapon_name() {
            let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
            if crate::game_logic::thing::ThingTemplate::weapon_from_store(name).is_some() {
                return crate::game_logic::host_armor_residual::resolve_host_death_type(
                    Some(name),
                    self.authored_damage_type(),
                );
            }
        }
        match self {
            HostSuperweaponKind::ParticleCannon => HostDeathType::Lasered,
            HostSuperweaponKind::NapalmStrike => HostDeathType::Burned,
            _ => HostDeathType::Exploded,
        }
    }
}

/// Residual `WeaponErrorRadius` scatter for artillery formation index.
///
/// C++ `DeliverPayloadNugget` (ObjectCreationList.cpp):
/// ```text
/// if (m_errorRadius > 1.0f && formationIndex > 0) {
///   randomRadius = GameLogicRandomValueReal(0, m_errorRadius);
///   randomAngle  = GameLogicRandomValueReal(0, PI*2);
///   targetPos.x += randomRadius * Cos(randomAngle);
///   targetPos.y += randomRadius * Sin(randomAngle);
/// }
/// ```
/// First formation slot is always spot-on (click target). Host residual uses
/// pure ADC RandomValue algorithm seeded by formation index (re-query stable
/// for multi-strike plan_due recomputes; algorithm parity with
/// GameLogicRandomValueReal — not golden-ratio residual).
/// C++ X/Y horizontal map to host X/Z.
pub fn weapon_error_radius_offset(formation_index: u32, error_radius: f32) -> Vec3 {
    if formation_index == 0 || error_radius <= 1.0 {
        return Vec3::ZERO;
    }
    use crate::game_logic::host_rng_residual::HostRandomState;
    let mut s = HostRandomState::seeded(formation_index.wrapping_add(1));
    let radius = s.next_real(0.0, error_radius);
    let angle = s.next_real(0.0, std::f32::consts::TAU);
    Vec3::new(radius * angle.cos(), 0.0, radius * angle.sin())
}

/// Residual `DelayDeliveryMax` frames for artillery formation index.
///
/// C++: `setDisabledUntil(frame + GameLogicRandomValue(0, m_delayDeliveryFramesMax))`
/// (inclusive integer range). Host residual: formationIndex 0 always 0 (lead
/// shell starts after base approach residual); remaining shells draw via pure
/// ADC RandomValue algorithm in `[0, max_frames]` inclusive.
pub fn delay_delivery_frames(formation_index: u32, max_frames: u32) -> u32 {
    if formation_index == 0 || max_frames == 0 {
        return 0;
    }
    use crate::game_logic::host_rng_residual::HostRandomState;
    let mut s = HostRandomState::seeded(formation_index.wrapping_add(0xD1));
    // Inclusive [0, max_frames] like GameLogicRandomValue(0, max).
    s.next_int(0, max_frames as i32).max(0) as u32
}

/// Absolute impact frame for artillery shell `formation_index`.
///
/// Base approach residual (`DelayDeliveryMax` as unified reaction) + per-shell
/// DelayDelivery stagger residual.
pub fn artillery_shell_impact_frame(activate_frame: u32, formation_index: u32) -> u32 {
    activate_frame
        .saturating_add(ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES)
        .saturating_add(delay_delivery_frames(
            formation_index,
            ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES,
        ))
}

/// Absolute impact frame for carpet bomb index `i` (DropDelay stagger residual).
///
/// First bomb at approach residual; subsequent bombs every `CARPET_BOMB_DROP_DELAY_FRAMES`.
pub fn carpet_bomb_impact_frame(activate_frame: u32, bomb_index: u32) -> u32 {
    carpet_bomb_impact_frame_for_tier(activate_frame, bomb_index, CarpetBombFactionTier::America)
}

/// Absolute impact frame for carpet bomb index with faction DropDelay residual.
pub fn carpet_bomb_impact_frame_for_tier(
    activate_frame: u32,
    bomb_index: u32,
    tier: CarpetBombFactionTier,
) -> u32 {
    activate_frame
        .saturating_add(CARPET_BOMB_IMPACT_DELAY_FRAMES)
        .saturating_add(bomb_index.saturating_mul(tier.drop_delay_frames()))
}

/// Last absolute multi-strike impact frame for a kind (complete residual).
pub fn multi_strike_last_impact_frame(
    kind: HostSuperweaponKind,
    activate_frame: u32,
    artillery_tier: ArtilleryBarrageScienceTier,
) -> u32 {
    if kind.is_scatter_multi_strike() {
        let count = artillery_tier.formation_size().max(1);
        (0..count)
            .map(|i| artillery_shell_impact_frame(activate_frame, i))
            .max()
            .unwrap_or_else(|| activate_frame.saturating_add(ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES))
    } else if kind.is_line_multi_strike() {
        // Default America Payload 15 residual (tier-aware path uses ocl_shell_frames).
        carpet_bomb_impact_frame(activate_frame, CARPET_BOMB_COUNT.saturating_sub(1))
    } else if kind.is_scud_multi_strike() {
        scud_missile_impact_frame(activate_frame, SCUD_STORM_MISSILE_COUNT.saturating_sub(1))
    } else {
        activate_frame.saturating_add(kind.impact_delay_frames())
    }
}

/// Residual DropVariance scatter for bomb index `i`.
///
/// C++ DeliverPayloadAIUpdate:
/// `pos.x += GameLogicRandomValueReal(-var.x, var.x);` (same for y/z when > 0).
/// Host residual: pure ADC RandomValue algorithm seeded by bomb index
/// (re-query stable; algorithm parity with GameLogicRandomValueReal).
/// C++ X/Y horizontal map to host X/Z; C++ Z maps to host Y (vertical).
pub fn drop_variance_offset(index: u32, var_x: f32, var_y: f32, var_z: f32) -> Vec3 {
    use crate::game_logic::host_rng_residual::HostRandomState;
    let mut s = HostRandomState::seeded(index.wrapping_add(1));
    let fx = if var_x > 0.0 {
        s.next_real(-var_x, var_x)
    } else {
        0.0
    };
    let fy = if var_y > 0.0 {
        s.next_real(-var_y, var_y)
    } else {
        0.0
    };
    let fz = if var_z > 0.0 {
        s.next_real(-var_z, var_z)
    } else {
        0.0
    };
    // Host Y-up: C++ X → X, C++ Y → Z, C++ Z → Y.
    Vec3::new(fx, fz, fy)
}

/// Unit XZ approach from `launch` toward the click (C++ DeliverPayload flight).
/// Degenerate / coincident points fall back to +X.
pub fn carpet_bomb_approach_dir(launch: Vec3, target: Vec3) -> (f32, f32) {
    let dx = target.x - launch.x;
    let dz = target.z - launch.z;
    let len = (dx * dx + dz * dz).sqrt();
    if len > 1.0e-4 {
        (dx / len, dz / len)
    } else {
        (1.0, 0.0)
    }
}

/// Build residual bomb epicenters along a line centered on `target`.
///
/// Default orientation is +X when no launch is known. Live flight uses
/// [`carpet_bomb_points_for_tier_along`] so the line follows the bomber
/// approach (C++ drops at transport position along the flight path).
/// Line length is `(count - 1) * CARPET_BOMB_SPACING` centered on target.
/// Each point applies retail DropVariance residual (X:30 Y:40 Z:0) via
/// deterministic host scatter (fail-closed vs GameLogicRandomValueReal).
pub fn carpet_bomb_points(target: Vec3) -> Vec<Vec3> {
    carpet_bomb_points_for_tier(target, CarpetBombFactionTier::America)
}

/// Build residual carpet bomb epicenters for a faction Payload/DropVariance residual.
///
/// Retail: USA Payload **15** / AirF **12** / China **10**. DropVariance always
/// X:30 Y:40 Z:0. PreferredHeight honesty on tier. Axis is +X unless the
/// caller supplies launch via [`carpet_bomb_points_for_tier_along`].
pub fn carpet_bomb_points_for_tier(target: Vec3, tier: CarpetBombFactionTier) -> Vec<Vec3> {
    carpet_bomb_points_for_tier_along(target, tier, Vec3::new(target.x - 1.0, target.y, target.z))
}

/// Bomb epicenters along `launch → target` (C++ DeliverPayload approach).
///
/// Index 0 is on the inbound side of the click; last index is past the
/// target along the same heading. DropVariance stays in world XZ (C++ XY).
pub fn carpet_bomb_points_for_tier_along(
    target: Vec3,
    tier: CarpetBombFactionTier,
    launch: Vec3,
) -> Vec<Vec3> {
    let count = tier.bomb_count().max(1);
    let half = (count as f32 - 1.0) * 0.5;
    let (ux, uz) = carpet_bomb_approach_dir(launch, target);
    let mut points = Vec::with_capacity(count as usize);
    for i in 0..count {
        let offset = (i as f32 - half) * CARPET_BOMB_SPACING;
        let scatter = drop_variance_offset(
            i,
            CARPET_BOMB_DROP_VARIANCE_X,
            CARPET_BOMB_DROP_VARIANCE_Y,
            CARPET_BOMB_DROP_VARIANCE_Z,
        );
        points.push(Vec3::new(
            target.x + ux * offset + scatter.x,
            target.y + scatter.y,
            target.z + uz * offset + scatter.z,
        ));
    }
    points
}

/// Build residual artillery shell epicenters scattered around `target`.
///
/// Formation index 0 is spot-on at the click target (C++). Remaining shells use
/// deterministic `WeaponErrorRadius` residual scatter in `[0, error_radius]`.
/// Shell count is Level1 FormationSize **12** by default.
pub fn artillery_barrage_points(target: Vec3) -> Vec<Vec3> {
    artillery_barrage_points_for_tier(target, ArtilleryBarrageScienceTier::Level1)
}

/// Build residual artillery shell epicenters for a science-tier FormationSize.
///
/// Retail: SUPERWEAPON_ArtilleryBarrage1/2/3 → FormationSize **12 / 24 / 36**.
/// Placement matches C++ `WeaponErrorRadius` residual (not fixed ring).
pub fn artillery_barrage_points_for_tier(
    target: Vec3,
    tier: ArtilleryBarrageScienceTier,
) -> Vec<Vec3> {
    let count = tier.formation_size().max(1);
    let mut points = Vec::with_capacity(count as usize);
    for i in 0..count {
        let off = weapon_error_radius_offset(i, ARTILLERY_BARRAGE_ERROR_RADIUS);
        points.push(Vec3::new(
            target.x + off.x,
            target.y + off.y,
            target.z + off.z,
        ));
    }
    points
}

/// Residual DelayBetweenShots frames for ScudStorm missile index.
///
/// Retail: DelayBetweenShots Min:100 Max:1000 (ms). Host residual: missile 0
/// has no inter-shot delay (PreAttack covers first); remaining missiles draw
/// via pure ADC GameLogicRandomValue algorithm in [min, max] inclusive frames.
pub fn scud_delay_between_frames(missile_index: u32) -> u32 {
    if missile_index == 0 {
        return 0;
    }
    use crate::game_logic::host_rng_residual::HostRandomState;
    let min = SCUD_STORM_DELAY_BETWEEN_MIN_FRAMES as i32;
    let max = SCUD_STORM_DELAY_BETWEEN_MAX_FRAMES as i32;
    let mut s = HostRandomState::seeded(missile_index.wrapping_add(0x5C1D));
    s.next_int(min, max).max(0) as u32
}

/// Absolute impact frame for ScudStorm missile `missile_index`.
///
/// Base PreAttackDelay residual + cumulative DelayBetweenShots stagger.
pub fn scud_missile_impact_frame(activate_frame: u32, missile_index: u32) -> u32 {
    let mut frame = activate_frame.saturating_add(SCUD_STORM_PRE_ATTACK_FRAMES);
    for i in 1..=missile_index {
        frame = frame.saturating_add(scud_delay_between_frames(i));
    }
    frame
}

/// Build residual ScudStorm missile epicenters from retail ScatterTarget table.
///
/// C++: targetPos += ScatterTarget[i] * ScatterTargetScalar (X/Y horizontal).
/// Host residual: C++ X → X, C++ Y → Z. ClipSize 9 entries.
pub fn scud_storm_points(target: Vec3) -> Vec<Vec3> {
    let mut points = Vec::with_capacity(SCUD_STORM_MISSILE_COUNT as usize);
    for (i, &(sx, sy)) in SCUD_STORM_SCATTER_TARGETS.iter().enumerate() {
        if i as u32 >= SCUD_STORM_MISSILE_COUNT {
            break;
        }
        points.push(Vec3::new(
            target.x + sx * SCUD_STORM_SCATTER_SCALAR,
            target.y,
            target.z + sy * SCUD_STORM_SCATTER_SCALAR,
        ));
    }
    // Pad if table shorter than clip (retail falls back to 0,0 for extras).
    while (points.len() as u32) < SCUD_STORM_MISSILE_COUNT {
        points.push(target);
    }
    points
}
