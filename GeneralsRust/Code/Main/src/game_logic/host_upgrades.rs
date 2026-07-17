//! Host upgrade queue/complete residual.
//!
//! Residual slice: `QueueUpgrade` → research complete → unlocks something
//! **observable** on host GameLogic:
//! - Capture building research unlocks infantry capture ability
//! - FlashBang research equips Ranger secondary grenade + upgrade tag
//! - TOW Missile research equips Humvee secondary + upgrade tag
//! - Supply Lines research tags supply centers **and** grants residual
//!   drop-off cash boost (economy residual; C++ Chinook `UpgradedSupplyBoost`)
//!
//! Wave 62 residual pack (retail Upgrade.ini cost/time + stealth forbidden):
//! - Retail BuildCost / BuildTime residual honesty for HostUpgradeKind
//!   (SupplyLines 800/30s, FlashBang 800/30s, TOW 800/30s, Capture 1000/30s,
//!   CompositeArmor 2000/60s, Camouflage 2000/60s, CamoNetting 500/5s,
//!   NeutronShells 2500/60s, NuclearTanks 2000/60s, …)
//! - Host research frames remain fail-closed **1** (observable queue) while
//!   `retail_research_frames` documents Upgrade.ini BuildTime → frames
//! - StealthForbiddenConditions residual: Camouflage Rebel = ATTACKING
//!   USING_ABILITY; CamoNetting structures = ATTACKING USING_ABILITY TAKING_DAMAGE
//!
//! Fail-closed: not full science tree, ProductionUpdate build-time parity,
//! WeaponSetUpgrade module matrix, full per-unit INI `UpgradedSupplyBoost`
//! matrix (Chinook 60), or multiplayer upgrade replication.
//! WorkerShoes residual lives in `host_gla_worker` (speed + supply boost 8).

use super::{ObjectId, Team};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Retail / host upgrade name constants used by residual effects.
pub const UPGRADE_INFANTRY_CAPTURE: &str = "Upgrade_InfantryCaptureBuilding";
pub const UPGRADE_AMERICA_RANGER_CAPTURE: &str = "Upgrade_AmericaRangerCaptureBuilding";
pub const UPGRADE_CHINA_REDGUARD_CAPTURE: &str = "Upgrade_ChinaRedguardCaptureBuilding";
pub const UPGRADE_GLA_REBEL_CAPTURE: &str = "Upgrade_GLARebelCaptureBuilding";
pub const UPGRADE_AMERICA_FLASHBANG: &str = "Upgrade_AmericaRangerFlashBangGrenade";
pub const UPGRADE_AMERICA_TOW: &str = "Upgrade_AmericaTOWMissile";
pub const UPGRADE_AMERICA_SUPPLY_LINES: &str = "Upgrade_AmericaSupplyLines";
pub const UPGRADE_CHINA_NEUTRON_SHELLS: &str = "Upgrade_ChinaNeutronShells";
pub const UPGRADE_AMERICA_BUNKER_BUSTERS: &str = "Upgrade_AmericaBunkerBusters";
pub const UPGRADE_COMANCHE_ROCKET_PODS: &str = "Upgrade_ComancheRocketPods";
pub const UPGRADE_AMERICA_SENTRY_DRONE_GUN: &str = "Upgrade_AmericaSentryDroneGun";
pub const UPGRADE_AMERICA_COMPOSITE_ARMOR: &str = "Upgrade_AmericaCompositeArmor";
pub const UPGRADE_GLA_WORKER_SHOES: &str = "Upgrade_GLAWorkerShoes";
pub const UPGRADE_CHINA_NUCLEAR_TANKS: &str = "Upgrade_ChinaNuclearTanks";
pub const UPGRADE_GLA_REBEL_BOOBY_TRAP: &str = "Upgrade_GLAInfantryRebelBoobyTrapAttack";
/// Demo General SuicideBomb death-weapon residual.
pub const UPGRADE_DEMO_SUICIDE_BOMB: &str = "Demo_Upgrade_SuicideBomb";
/// America Advanced Control Rods residual (PowerPlantUpgrade EnergyBonus).
pub const UPGRADE_AMERICA_ADVANCED_CONTROL_RODS: &str = "Upgrade_AmericaAdvancedControlRods";
/// China Subliminal Messaging residual (propaganda tower upgrade).
pub const UPGRADE_CHINA_SUBLIMINAL_MESSAGING: &str = "Upgrade_ChinaSubliminalMessaging";
/// GLA Scorpion Rocket residual.
pub const UPGRADE_GLA_SCORPION_ROCKET: &str = "Upgrade_GLAScorpionRocket";
/// GLA AP Rockets residual.
pub const UPGRADE_GLA_AP_ROCKETS: &str = "Upgrade_GLAAPRockets";
/// America Laser Missiles residual.
pub const UPGRADE_AMERICA_LASER_MISSILES: &str = "Upgrade_AmericaLaserMissiles";
/// China Nationalism residual.
pub const UPGRADE_CHINA_NATIONALISM: &str = "Upgrade_ChinaNationalism";
/// China Chain Guns residual.
pub const UPGRADE_CHINA_CHAIN_GUNS: &str = "Upgrade_ChinaChainGuns";
/// China Uranium Shells residual.
pub const UPGRADE_CHINA_URANIUM_SHELLS: &str = "Upgrade_ChinaUraniumShells";
/// China Black Napalm residual.
pub const UPGRADE_CHINA_BLACK_NAPALM: &str = "Upgrade_ChinaBlackNapalm";
/// GLA AP Bullets residual.
pub const UPGRADE_GLA_AP_BULLETS: &str = "Upgrade_GLAAPBullets";
/// GLA Anthrax Beta residual.
pub const UPGRADE_GLA_ANTHRAX_BETA: &str = "Upgrade_GLAAnthraxBeta";
/// GLA Toxin Shells residual.
pub const UPGRADE_GLA_TOXIN_SHELLS: &str = "Upgrade_GLAToxinShells";

/// Residual drop-off cash boost when Supply Lines is unlocked for the player.
///
/// Matches C++ America Chinook `UpgradedSupplyBoost = 60` applied once per
/// SupplyCenter dock action (`SupplyCenterDockUpdate::action` +
/// `ChinookAIUpdate::getUpgradedSupplyBoost`). Host residual applies this
/// player-level flat boost on resource return deposit (not per-box, not
/// per-template matrix).
pub const SUPPLY_LINES_RESIDUAL_DROP_OFF_BOOST: u32 = 60;

/// Normalize upgrade identity the same way Player does (alphanumeric lower).
pub fn normalize_upgrade_identity(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Residual cash added on a supply-center deposit when Supply Lines is active.
/// Fail-closed: 0 when the upgrade is not unlocked.
pub fn residual_supply_lines_drop_off_boost(has_supply_lines: bool) -> u32 {
    if has_supply_lines {
        SUPPLY_LINES_RESIDUAL_DROP_OFF_BOOST
    } else {
        0
    }
}

/// Host residual upgrade kinds with known observable unlocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostUpgradeKind {
    /// Unlocks CaptureBuilding special ability for infantry.
    CaptureBuilding,
    /// Equips Ranger SECONDARY flashbang grenade weapon set.
    FlashBangGrenade,
    /// Equips Humvee SECONDARY TOW missile weapon set.
    TowMissile,
    /// Supply-lines research: tags supply centers + residual drop-off cash boost.
    SupplyLines,
    /// China Neutron Shells: equips Nuke Cannon SECONDARY neutron blast weapon.
    NeutronShells,
    /// America Bunker Busters: tags Stealth Fighters for residual bunker bust.
    BunkerBusters,
    /// America Comanche Rocket Pods: equips residual rocket-pod secondary + area attack.
    ComancheRocketPods,
    /// America Sentry Drone Gun: equips residual SentryDroneGun primary for auto-fire.
    SentryDroneGun,
    /// GLA Camouflage: grants residual stealth to Rebel infantry (StealthUpgrade).
    Camouflage,
    /// GLA CamoNetting: grants residual stealth to GLA structures (StealthUpgrade).
    /// Stealth General buildings + Tunnel Network / Stinger Site residual.
    CamoNetting,
    /// America Composite Armor: +100 MaxHealth on Crusader / Paladin residual.
    CompositeArmor,
    /// GLA WorkerShoes: speed boost + supply drop-off cash residual.
    WorkerShoes,
    /// China Nuclear Tanks: death blast + nuclear locomotor speed residual.
    NuclearTanks,
    /// GLA Rebel BoobyTrap attack unlock residual.
    BoobyTrap,
    /// Chem Anthrax Gamma: toxin combat upgrade residual (stream/field DoT).
    AnthraxGamma,
    /// Demo SuicideBomb: structure/unit death blast residual.
    SuicideBomb,
    /// America Advanced Control Rods: EnergyBonus on power plants.
    AdvancedControlRods,
    /// China Subliminal Messaging: upgraded propaganda heal/buff residual.
    SubliminalMessaging,
    /// GLA Scorpion Rocket secondary missile residual.
    ScorpionRocket,
    /// GLA AP Rockets damage residual (Scorpion / RPG).
    ApRockets,
    /// America Laser Missiles jet damage residual.
    LaserMissiles,
    /// China Nationalism horde ROF residual.
    Nationalism,
    /// China Chain Guns gattling/minigun damage residual.
    ChainGuns,
    /// China Uranium Shells tank damage residual.
    UraniumShells,
    /// China Black Napalm fire-field residual.
    BlackNapalm,
    /// GLA AP Bullets damage residual (Rebel/Jarmen/Technical/Quad).
    ApBullets,
    /// GLA Anthrax Beta toxin upgrade residual.
    AnthraxBeta,
    /// GLA Toxin Shells SCUD secondary residual.
    ToxinShells,
    /// Other / unknown upgrades (unlock flag only).
    Other,
}

impl HostUpgradeKind {
    /// Classify an upgrade name into a residual kind.
    pub fn from_name(name: &str) -> Self {
        let n = normalize_upgrade_identity(name);
        if n.contains("capturebuilding") || n.contains("infantrycapture") {
            HostUpgradeKind::CaptureBuilding
        } else if n.contains("flashbang") {
            HostUpgradeKind::FlashBangGrenade
        } else if n.contains("towmissile") || n == "upgradeamericatowmissile" {
            HostUpgradeKind::TowMissile
        } else if n.contains("supplylines") {
            HostUpgradeKind::SupplyLines
        } else if n.contains("neutronshells") || n.contains("neutronshell") {
            HostUpgradeKind::NeutronShells
        } else if n.contains("bunkerbuster") || n.contains("bunkerbusters") {
            HostUpgradeKind::BunkerBusters
        } else if n.contains("comancherocketpod") || n.contains("rocketpods") {
            HostUpgradeKind::ComancheRocketPods
        } else if n.contains("sentrydronegun") || n.contains("sentrydrone") {
            HostUpgradeKind::SentryDroneGun
        } else if n.contains("camonetting") || n.contains("camoneting") {
            // Stealth General structure CamoNetting (must precede camouflage? no overlap).
            HostUpgradeKind::CamoNetting
        } else if n.contains("camouflage") || n.contains("camoflage") {
            // Retail spelling is Camouflage; allow common misspelling residual.
            HostUpgradeKind::Camouflage
        } else if n.contains("anthraxgamma") {
            HostUpgradeKind::AnthraxGamma
        } else if n.contains("suicidebomb") || n.contains("demosuicidebomb") {
            HostUpgradeKind::SuicideBomb
        } else if n.contains("compositearmor") || n.contains("compositearmour") {
            HostUpgradeKind::CompositeArmor
        } else if n.contains("workershoes") || n.contains("glaworkershoes") {
            HostUpgradeKind::WorkerShoes
        } else if n.contains("nucleartanks") || n.contains("nucleartank") {
            HostUpgradeKind::NuclearTanks
        } else if n.contains("boobytrap") {
            HostUpgradeKind::BoobyTrap
        } else if n.contains("advancedcontrolrods") || n.contains("controlrods") {
            HostUpgradeKind::AdvancedControlRods
        } else if n.contains("subliminalmessaging") || n.contains("subliminal") {
            HostUpgradeKind::SubliminalMessaging
        } else if n.contains("scorpionrocket") {
            HostUpgradeKind::ScorpionRocket
        } else if n.contains("aprockets") || n.contains("aprocket") {
            HostUpgradeKind::ApRockets
        } else if n.contains("lasermissiles") || n.contains("lasermissile") {
            HostUpgradeKind::LaserMissiles
        } else if n.contains("nationalism") {
            HostUpgradeKind::Nationalism
        } else if n.contains("chainguns") || n.contains("chaingun") {
            HostUpgradeKind::ChainGuns
        } else if n.contains("uraniumshells") || n.contains("uraniumshell") {
            HostUpgradeKind::UraniumShells
        } else if n.contains("blacknapalm") {
            HostUpgradeKind::BlackNapalm
        } else if n.contains("apbullets") || n.contains("apbullet") {
            HostUpgradeKind::ApBullets
        } else if n.contains("anthraxbeta") {
            HostUpgradeKind::AnthraxBeta
        } else if n.contains("toxinshells") || n.contains("toxinshell") {
            HostUpgradeKind::ToxinShells
        } else {
            HostUpgradeKind::Other
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            HostUpgradeKind::CaptureBuilding => "CaptureBuilding",
            HostUpgradeKind::FlashBangGrenade => "FlashBangGrenade",
            HostUpgradeKind::TowMissile => "TowMissile",
            HostUpgradeKind::SupplyLines => "SupplyLines",
            HostUpgradeKind::NeutronShells => "NeutronShells",
            HostUpgradeKind::BunkerBusters => "BunkerBusters",
            HostUpgradeKind::ComancheRocketPods => "ComancheRocketPods",
            HostUpgradeKind::SentryDroneGun => "SentryDroneGun",
            HostUpgradeKind::Camouflage => "Camouflage",
            HostUpgradeKind::CamoNetting => "CamoNetting",
            HostUpgradeKind::CompositeArmor => "CompositeArmor",
            HostUpgradeKind::WorkerShoes => "WorkerShoes",
            HostUpgradeKind::NuclearTanks => "NuclearTanks",
            HostUpgradeKind::BoobyTrap => "BoobyTrap",
            HostUpgradeKind::AnthraxGamma => "AnthraxGamma",
            HostUpgradeKind::SuicideBomb => "SuicideBomb",
            HostUpgradeKind::AdvancedControlRods => "AdvancedControlRods",
            HostUpgradeKind::SubliminalMessaging => "SubliminalMessaging",
            HostUpgradeKind::ScorpionRocket => "ScorpionRocket",
            HostUpgradeKind::ApRockets => "ApRockets",
            HostUpgradeKind::LaserMissiles => "LaserMissiles",
            HostUpgradeKind::Nationalism => "Nationalism",
            HostUpgradeKind::ChainGuns => "ChainGuns",
            HostUpgradeKind::UraniumShells => "UraniumShells",
            HostUpgradeKind::BlackNapalm => "BlackNapalm",
            HostUpgradeKind::ApBullets => "ApBullets",
            HostUpgradeKind::AnthraxBeta => "AnthraxBeta",
            HostUpgradeKind::ToxinShells => "ToxinShells",
            HostUpgradeKind::Other => "Other",
        }
    }

    /// Residual research frames before complete (host path).
    /// `0` / `1` ⇒ complete on the next `GameLogic::update` (queue still observable).
    /// Fail-closed host path: not retail Upgrade.ini BuildTime (see
    /// [`Self::retail_research_frames`] for honesty pack).
    pub fn residual_research_frames(self) -> u32 {
        match self {
            // One logic update of research so QueueUpgrade is observably pending.
            HostUpgradeKind::CaptureBuilding
            | HostUpgradeKind::FlashBangGrenade
            | HostUpgradeKind::TowMissile
            | HostUpgradeKind::SupplyLines
            | HostUpgradeKind::NeutronShells
            | HostUpgradeKind::BunkerBusters
            | HostUpgradeKind::ComancheRocketPods
            | HostUpgradeKind::SentryDroneGun
            | HostUpgradeKind::Camouflage
            | HostUpgradeKind::CamoNetting
            | HostUpgradeKind::CompositeArmor
            | HostUpgradeKind::WorkerShoes
            | HostUpgradeKind::NuclearTanks
            | HostUpgradeKind::BoobyTrap
            | HostUpgradeKind::AnthraxGamma
            | HostUpgradeKind::SuicideBomb
            | HostUpgradeKind::AdvancedControlRods
            | HostUpgradeKind::SubliminalMessaging
            | HostUpgradeKind::ScorpionRocket
            | HostUpgradeKind::ApRockets
            | HostUpgradeKind::LaserMissiles
            | HostUpgradeKind::Nationalism
            | HostUpgradeKind::ChainGuns
            | HostUpgradeKind::UraniumShells
            | HostUpgradeKind::BlackNapalm
            | HostUpgradeKind::ApBullets
            | HostUpgradeKind::AnthraxBeta
            | HostUpgradeKind::ToxinShells
            | HostUpgradeKind::Other => 1,
        }
    }

    /// Retail Upgrade.ini BuildCost residual (cash).
    ///
    /// Fail-closed: `Other` / unknown → **0**. Host research path still uses
    /// short residual frames; this is honesty of retail cost matrix.
    pub fn retail_build_cost(self) -> u32 {
        match self {
            HostUpgradeKind::CaptureBuilding => 1000, // Upgrade_AmericaRangerCaptureBuilding
            HostUpgradeKind::FlashBangGrenade => 800,
            HostUpgradeKind::TowMissile => 800,
            HostUpgradeKind::SupplyLines => 800,
            HostUpgradeKind::NeutronShells => 2500,
            HostUpgradeKind::BunkerBusters => 1500,
            HostUpgradeKind::ComancheRocketPods => 800,
            HostUpgradeKind::SentryDroneGun => 1000,
            HostUpgradeKind::Camouflage => 2000,
            HostUpgradeKind::CamoNetting => 500,
            HostUpgradeKind::CompositeArmor => 2000,
            HostUpgradeKind::WorkerShoes => 1000,
            HostUpgradeKind::NuclearTanks => 2000,
            HostUpgradeKind::BoobyTrap => 1000,
            HostUpgradeKind::AnthraxGamma => 1500,
            HostUpgradeKind::SuicideBomb => 2000,
            HostUpgradeKind::AdvancedControlRods => 1500,
            HostUpgradeKind::SubliminalMessaging => 2000,
            HostUpgradeKind::ScorpionRocket => 800,
            HostUpgradeKind::ApRockets => 1500,
            HostUpgradeKind::LaserMissiles => 1500,
            HostUpgradeKind::Nationalism => 2000,
            HostUpgradeKind::ChainGuns => 1500,
            HostUpgradeKind::UraniumShells => 2000,
            HostUpgradeKind::BlackNapalm => 2000,
            HostUpgradeKind::ApBullets => 1500,
            HostUpgradeKind::AnthraxBeta => 2000,
            HostUpgradeKind::ToxinShells => 1000,
            HostUpgradeKind::Other => 0,
        }
    }

    /// Retail Upgrade.ini BuildTime residual (seconds).
    pub fn retail_build_time_secs(self) -> f32 {
        match self {
            HostUpgradeKind::CaptureBuilding => 30.0,
            HostUpgradeKind::FlashBangGrenade => 30.0,
            HostUpgradeKind::TowMissile => 30.0,
            HostUpgradeKind::SupplyLines => 30.0,
            HostUpgradeKind::NeutronShells => 60.0,
            HostUpgradeKind::BunkerBusters => 40.0,
            HostUpgradeKind::ComancheRocketPods => 40.0,
            HostUpgradeKind::SentryDroneGun => 30.0,
            HostUpgradeKind::Camouflage => 60.0,
            HostUpgradeKind::CamoNetting => 5.0,
            HostUpgradeKind::CompositeArmor => 60.0,
            HostUpgradeKind::WorkerShoes => 10.0,
            HostUpgradeKind::NuclearTanks => 60.0,
            HostUpgradeKind::BoobyTrap => 30.0,
            HostUpgradeKind::AnthraxGamma => 60.0,
            HostUpgradeKind::SuicideBomb => 30.0,
            HostUpgradeKind::AdvancedControlRods => 60.0,
            HostUpgradeKind::SubliminalMessaging => 60.0,
            HostUpgradeKind::ScorpionRocket => 30.0,
            HostUpgradeKind::ApRockets => 45.0,
            HostUpgradeKind::LaserMissiles => 40.0,
            HostUpgradeKind::Nationalism => 60.0,
            HostUpgradeKind::ChainGuns => 40.0,
            HostUpgradeKind::UraniumShells => 60.0,
            HostUpgradeKind::BlackNapalm => 60.0,
            HostUpgradeKind::ApBullets => 45.0,
            HostUpgradeKind::AnthraxBeta => 60.0,
            HostUpgradeKind::ToxinShells => 30.0,
            HostUpgradeKind::Other => 0.0,
        }
    }

    /// Retail Upgrade.ini BuildTime → frames @ 30 FPS.
    pub fn retail_research_frames(self) -> u32 {
        (self.retail_build_time_secs() * 30.0).round() as u32
    }
}

/// Lifecycle of a host residual upgrade research entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostUpgradePhase {
    /// Queued after QueueUpgrade; waiting for research complete.
    Queued,
    /// Research finished; unlock effects applied.
    Completed,
    /// Cancelled / refunded before complete.
    Cancelled,
}

/// One queued or completed host upgrade research record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostUpgradeResearch {
    pub id: u32,
    pub name: String,
    pub kind: HostUpgradeKind,
    pub team: Team,
    pub player_id: u32,
    pub queue_frame: u32,
    pub complete_frame: u32,
    pub phase: HostUpgradePhase,
    /// Units that received an observable effect (weapon equip / upgrade tag).
    pub units_affected: u32,
    /// Producer building (optional residual).
    pub source_object: Option<ObjectId>,
    /// Wave 79: cash supplies paid at QueueUpgrade (retail cost residual application).
    #[serde(default)]
    pub build_cost_paid: u32,
    /// Wave 79: retail Upgrade.ini BuildTime → frames residual bookkeeping.
    #[serde(default)]
    pub retail_research_frames: u32,
    /// Wave 79: host residual research frames (still **1** for observable queue).
    #[serde(default = "default_residual_research_frames")]
    pub residual_research_frames: u32,
}

fn default_residual_research_frames() -> u32 {
    1
}

/// Host registry of upgrade research queue/complete for honesty + residual apply.
#[derive(Debug, Clone, Default)]
pub struct HostUpgradeRegistry {
    next_id: u32,
    /// Active + completed research keyed by id.
    entries: HashMap<u32, HostUpgradeResearch>,
    /// Pending lookup: (player_id, normalized name) → entry id.
    pending_index: HashMap<(u32, String), u32>,
    completed_this_frame: Vec<u32>,
    queued_this_frame: Vec<u32>,
}

impl HostUpgradeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::new();
    }

    pub fn clear_frame_events(&mut self) {
        self.completed_this_frame.clear();
        self.queued_this_frame.clear();
    }

    /// Snapshot research completed this logic frame (for PresentationFrame freeze).
    /// Does not drain — cleared at next `clear_frame_events` / update start.
    pub fn completed_this_frame_snapshot(&self) -> Vec<HostUpgradeResearch> {
        self.completed_this_frame
            .iter()
            .filter_map(|id| self.entries.get(id).cloned())
            .collect()
    }

    /// Snapshot research queued this logic frame (presentation residual).
    pub fn queued_this_frame_snapshot(&self) -> Vec<HostUpgradeResearch> {
        self.queued_this_frame
            .iter()
            .filter_map(|id| self.entries.get(id).cloned())
            .collect()
    }

    /// Next allocator id (for save/load residual).
    pub fn next_id(&self) -> u32 {
        self.next_id
    }

    /// Replace registry contents from a save/load snapshot.
    ///
    /// Frame-local presentation drains (`queued_this_frame` /
    /// `completed_this_frame`) are cleared — they are not persistent.
    /// Rebuilds `pending_index` from entries still in [`HostUpgradePhase::Queued`].
    pub fn restore_from_snapshot(
        &mut self,
        next_id: u32,
        entries: impl IntoIterator<Item = HostUpgradeResearch>,
    ) {
        self.clear();
        let mut max_id = 0_u32;
        for entry in entries {
            max_id = max_id.max(entry.id);
            if entry.phase == HostUpgradePhase::Queued {
                let key = (entry.player_id, normalize_upgrade_identity(&entry.name));
                self.pending_index.insert(key, entry.id);
            }
            self.entries.insert(entry.id, entry);
        }
        // Prefer the saved allocator; never reuse an id that is already present.
        self.next_id = next_id.max(max_id.saturating_add(1));
    }

    pub fn get(&self, id: u32) -> Option<&HostUpgradeResearch> {
        self.entries.get(&id)
    }

    pub fn pending_count(&self) -> usize {
        self.entries
            .values()
            .filter(|e| e.phase == HostUpgradePhase::Queued)
            .count()
    }

    pub fn entries_snapshot(&self) -> Vec<HostUpgradeResearch> {
        let mut v: Vec<_> = self.entries.values().cloned().collect();
        v.sort_by_key(|e| e.id);
        v
    }

    pub fn pending_of_kind(&self, kind: HostUpgradeKind) -> Vec<&HostUpgradeResearch> {
        self.entries
            .values()
            .filter(|e| e.kind == kind && e.phase == HostUpgradePhase::Queued)
            .collect()
    }

    pub fn completed_of_kind(&self, kind: HostUpgradeKind) -> Vec<&HostUpgradeResearch> {
        self.entries
            .values()
            .filter(|e| e.kind == kind && e.phase == HostUpgradePhase::Completed)
            .collect()
    }

    /// Record a newly queued upgrade research (idempotent per player+name).
    pub fn record_queue(
        &mut self,
        name: &str,
        team: Team,
        player_id: u32,
        frame: u32,
        source_object: Option<ObjectId>,
    ) -> u32 {
        let key = (player_id, normalize_upgrade_identity(name));
        if let Some(&existing) = self.pending_index.get(&key) {
            return existing;
        }
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let kind = HostUpgradeKind::from_name(name);
        let entry = HostUpgradeResearch {
            id,
            name: name.to_string(),
            kind,
            team,
            player_id,
            queue_frame: frame,
            complete_frame: 0,
            phase: HostUpgradePhase::Queued,
            units_affected: 0,
            source_object,
            // Cost is filled by [`Self::set_build_cost_paid`] when the command
            // path debits cash; default to retail matrix for residual honesty.
            build_cost_paid: kind.retail_build_cost(),
            retail_research_frames: kind.retail_research_frames(),
            residual_research_frames: kind.residual_research_frames(),
        };
        self.entries.insert(id, entry);
        self.pending_index.insert(key, id);
        self.queued_this_frame.push(id);
        id
    }

    /// Wave 79: record actual cash paid at QueueUpgrade (retail cost application).
    pub fn set_build_cost_paid(&mut self, name: &str, player_id: u32, cost: u32) {
        let key = (player_id, normalize_upgrade_identity(name));
        if let Some(&id) = self.pending_index.get(&key) {
            if let Some(entry) = self.entries.get_mut(&id) {
                entry.build_cost_paid = cost;
            }
        }
    }

    /// Mark research completed and store how many units were affected.
    pub fn record_complete(
        &mut self,
        name: &str,
        player_id: u32,
        frame: u32,
        units_affected: u32,
    ) -> Option<u32> {
        let key = (player_id, normalize_upgrade_identity(name));
        let id = if let Some(&id) = self.pending_index.get(&key) {
            id
        } else {
            // Complete without prior queue record (script grant path) — create completed entry.
            let id = self.next_id;
            self.next_id = self.next_id.saturating_add(1);
            let kind = HostUpgradeKind::from_name(name);
            self.entries.insert(
                id,
                HostUpgradeResearch {
                    id,
                    name: name.to_string(),
                    kind,
                    team: Team::Neutral,
                    player_id,
                    queue_frame: frame,
                    complete_frame: frame,
                    phase: HostUpgradePhase::Completed,
                    units_affected,
                    source_object: None,
                    build_cost_paid: kind.retail_build_cost(),
                    retail_research_frames: kind.retail_research_frames(),
                    residual_research_frames: kind.residual_research_frames(),
                },
            );
            self.completed_this_frame.push(id);
            return Some(id);
        };

        if let Some(entry) = self.entries.get_mut(&id) {
            entry.phase = HostUpgradePhase::Completed;
            entry.complete_frame = frame;
            entry.units_affected = units_affected;
        }
        self.pending_index.remove(&key);
        self.completed_this_frame.push(id);
        Some(id)
    }

    /// Most recent completed/queued upgrade residual source object for radar event.
    pub fn last_source_object_for(
        &self,
        player_id: u32,
        upgrade_name: &str,
    ) -> Option<crate::game_logic::ObjectId> {
        // Prefer highest-id matching research entry with a source.
        self.entries
            .values()
            .filter(|r| {
                r.player_id == player_id
                    && r.name.eq_ignore_ascii_case(upgrade_name)
                    && r.source_object.is_some()
            })
            .max_by_key(|r| r.id)
            .and_then(|r| r.source_object)
    }

    /// Cancel a pending research (refund path).
    pub fn record_cancel(&mut self, name: &str, player_id: u32) -> bool {
        let key = (player_id, normalize_upgrade_identity(name));
        let Some(id) = self.pending_index.remove(&key) else {
            return false;
        };
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.phase = HostUpgradePhase::Cancelled;
        }
        true
    }

    // --- Honesty flags (host residual; do not claim full retail parity) ---

    pub fn honesty_queue_ok(&self, kind: HostUpgradeKind) -> bool {
        !self.pending_of_kind(kind).is_empty()
    }

    pub fn honesty_complete_ok(&self, kind: HostUpgradeKind) -> bool {
        self.completed_of_kind(kind)
            .iter()
            .any(|e| e.phase == HostUpgradePhase::Completed)
    }

    /// Host path honesty: completed research with at least one observable unlock
    /// (units_affected > 0) **or** CaptureBuilding (ability unlock is player-flag only).
    pub fn honesty_host_path_ok(&self, kind: HostUpgradeKind) -> bool {
        self.completed_of_kind(kind).iter().any(|e| {
            e.phase == HostUpgradePhase::Completed
                && (e.units_affected > 0
                    || kind == HostUpgradeKind::CaptureBuilding
                    || kind == HostUpgradeKind::SupplyLines
                    || kind == HostUpgradeKind::Other)
        })
    }

    /// True if any completed FlashBang research equipped at least one unit.
    pub fn honesty_flashbang_equipped_ok(&self) -> bool {
        self.completed_of_kind(HostUpgradeKind::FlashBangGrenade)
            .iter()
            .any(|e| e.units_affected > 0)
    }

    /// True if Capture research completed (player unlock flag is the ability gate).
    pub fn honesty_capture_unlock_ok(&self) -> bool {
        self.honesty_complete_ok(HostUpgradeKind::CaptureBuilding)
    }

    /// True if Supply Lines research completed (tag residual; economy boost is
    /// tracked on GameLogic via `honesty_supply_lines_economy_ok`).
    pub fn honesty_supply_lines_complete_ok(&self) -> bool {
        self.honesty_complete_ok(HostUpgradeKind::SupplyLines)
    }
}

/// Template names that receive FlashBang secondary when research completes.
pub fn is_flashbang_unit_template(name: &str) -> bool {
    matches!(
        name,
        "USA_Ranger" | "GoldenRanger" | "AmericaInfantryRanger" | "TestRanger" | "TestInfantry"
    ) || {
        let n = name.to_ascii_lowercase();
        n.contains("ranger") && !n.contains("humvee")
    }
}

/// Template names that receive TOW secondary when research completes.
pub fn is_tow_unit_template(name: &str) -> bool {
    matches!(name, "USA_Humvee" | "AmericaVehicleHumvee" | "GoldenHumvee")
        || name.to_ascii_lowercase().contains("humvee")
}

/// Template names that receive Neutron Shell secondary when research completes.
pub fn is_neutron_shell_unit_template(name: &str) -> bool {
    crate::game_logic::host_neutron_shell::is_nuke_cannon_template(name)
}

/// Template names that receive Bunker Busters upgrade tag when research completes.
pub fn is_bunker_buster_unit_template(name: &str) -> bool {
    crate::game_logic::host_bunker_buster::is_bunker_buster_carrier(name)
}

/// Template names that receive Comanche Rocket Pods secondary when research completes.
pub fn is_comanche_rocket_pod_unit_template(name: &str) -> bool {
    crate::game_logic::host_comanche_rocket_pods::is_comanche_template(name)
}

/// Template names that receive Sentry Drone Gun primary when research completes.
pub fn is_sentry_drone_gun_unit_template(name: &str) -> bool {
    crate::game_logic::host_sentry_drone::is_sentry_drone_template(name)
}

/// Template names that receive Capture upgrade tag (observability residual).
pub fn is_capture_capable_infantry_template(name: &str) -> bool {
    if name.to_ascii_lowercase().contains("worker")
        || name.to_ascii_lowercase().contains("dozer")
        || name.to_ascii_lowercase().contains("pilot")
    {
        return false;
    }
    matches!(
        name,
        "USA_Ranger"
            | "GoldenRanger"
            | "AmericaInfantryRanger"
            | "GLA_Soldier"
            | "GLA_Rebel"
            | "GLAInfantryRebel"
            | "China_RedGuard"
            | "China_Soldier"
            | "ChinaInfantryRedguard"
            | "TestInfantry"
            | "TestRanger"
    ) || {
        let n = name.to_ascii_lowercase();
        n.contains("ranger")
            || n.contains("rebel")
            || n.contains("redguard")
            || n.contains("infantry")
    }
}

/// Supply-center templates for Supply Lines residual tag.
pub fn is_supply_center_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("supplycenter") || n.contains("supply_center") || name == "AmericaSupplyCenter"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_capture_flashbang_tow_supply() {
        assert_eq!(
            HostUpgradeKind::from_name(UPGRADE_INFANTRY_CAPTURE),
            HostUpgradeKind::CaptureBuilding
        );
        assert_eq!(
            HostUpgradeKind::from_name(UPGRADE_AMERICA_RANGER_CAPTURE),
            HostUpgradeKind::CaptureBuilding
        );
        assert_eq!(
            HostUpgradeKind::from_name(UPGRADE_AMERICA_FLASHBANG),
            HostUpgradeKind::FlashBangGrenade
        );
        assert_eq!(
            HostUpgradeKind::from_name("upgradeamericarangerflashbanggrenade"),
            HostUpgradeKind::FlashBangGrenade
        );
        assert_eq!(
            HostUpgradeKind::from_name(UPGRADE_AMERICA_TOW),
            HostUpgradeKind::TowMissile
        );
        assert_eq!(
            HostUpgradeKind::from_name(UPGRADE_AMERICA_SUPPLY_LINES),
            HostUpgradeKind::SupplyLines
        );
        assert_eq!(
            HostUpgradeKind::from_name(UPGRADE_CHINA_NEUTRON_SHELLS),
            HostUpgradeKind::NeutronShells
        );
        assert_eq!(
            HostUpgradeKind::from_name(UPGRADE_AMERICA_BUNKER_BUSTERS),
            HostUpgradeKind::BunkerBusters
        );
        assert_eq!(
            HostUpgradeKind::from_name(UPGRADE_COMANCHE_ROCKET_PODS),
            HostUpgradeKind::ComancheRocketPods
        );
        assert_eq!(
            HostUpgradeKind::from_name(UPGRADE_AMERICA_SENTRY_DRONE_GUN),
            HostUpgradeKind::SentryDroneGun
        );
        assert_eq!(
            HostUpgradeKind::from_name(UPGRADE_AMERICA_COMPOSITE_ARMOR),
            HostUpgradeKind::CompositeArmor
        );
        assert_eq!(
            HostUpgradeKind::from_name(UPGRADE_GLA_WORKER_SHOES),
            HostUpgradeKind::WorkerShoes
        );
        assert_eq!(
            HostUpgradeKind::from_name("Upgrade_ChinaNationalism"),
            HostUpgradeKind::Other
        );
    }

    #[test]
    fn registry_queue_complete_honesty() {
        let mut reg = HostUpgradeRegistry::new();
        let id = reg.record_queue(
            UPGRADE_AMERICA_FLASHBANG,
            Team::USA,
            0,
            10,
            Some(ObjectId(5)),
        );
        assert!(reg.honesty_queue_ok(HostUpgradeKind::FlashBangGrenade));
        assert!(!reg.honesty_complete_ok(HostUpgradeKind::FlashBangGrenade));
        assert_eq!(reg.get(id).unwrap().phase, HostUpgradePhase::Queued);

        reg.record_complete(UPGRADE_AMERICA_FLASHBANG, 0, 11, 2);
        assert!(!reg.honesty_queue_ok(HostUpgradeKind::FlashBangGrenade));
        assert!(reg.honesty_complete_ok(HostUpgradeKind::FlashBangGrenade));
        assert!(reg.honesty_flashbang_equipped_ok());
        assert!(reg.honesty_host_path_ok(HostUpgradeKind::FlashBangGrenade));
        assert_eq!(reg.get(id).unwrap().units_affected, 2);
        assert_eq!(reg.get(id).unwrap().phase, HostUpgradePhase::Completed);
    }

    #[test]
    fn capture_complete_honesty_without_unit_tags() {
        let mut reg = HostUpgradeRegistry::new();
        reg.record_queue(UPGRADE_INFANTRY_CAPTURE, Team::USA, 0, 1, None);
        reg.record_complete(UPGRADE_INFANTRY_CAPTURE, 0, 2, 0);
        assert!(reg.honesty_capture_unlock_ok());
        assert!(reg.honesty_host_path_ok(HostUpgradeKind::CaptureBuilding));
    }

    #[test]
    fn cancel_clears_pending() {
        let mut reg = HostUpgradeRegistry::new();
        reg.record_queue(UPGRADE_AMERICA_SUPPLY_LINES, Team::USA, 0, 1, None);
        assert!(reg.honesty_queue_ok(HostUpgradeKind::SupplyLines));
        assert!(reg.record_cancel(UPGRADE_AMERICA_SUPPLY_LINES, 0));
        assert!(!reg.honesty_queue_ok(HostUpgradeKind::SupplyLines));
    }

    #[test]
    fn supply_lines_drop_off_boost_fail_closed() {
        assert_eq!(residual_supply_lines_drop_off_boost(false), 0);
        assert_eq!(
            residual_supply_lines_drop_off_boost(true),
            SUPPLY_LINES_RESIDUAL_DROP_OFF_BOOST
        );
        assert!(SUPPLY_LINES_RESIDUAL_DROP_OFF_BOOST > 0);
    }

    #[test]
    fn restore_from_snapshot_keeps_pending_queue() {
        let mut reg = HostUpgradeRegistry::new();
        let id = reg.record_queue(
            UPGRADE_INFANTRY_CAPTURE,
            Team::USA,
            0,
            10,
            Some(ObjectId(7)),
        );
        assert_eq!(reg.pending_count(), 1);
        let snap = reg.entries_snapshot();
        let next = reg.next_id();

        let mut loaded = HostUpgradeRegistry::new();
        loaded.restore_from_snapshot(next, snap);
        assert_eq!(loaded.pending_count(), 1);
        let e = loaded.get(id).expect("restored research");
        assert_eq!(e.phase, HostUpgradePhase::Queued);
        assert_eq!(e.kind, HostUpgradeKind::CaptureBuilding);
        assert_eq!(e.queue_frame, 10);
        assert_eq!(e.source_object, Some(ObjectId(7)));
        assert_eq!(loaded.next_id(), next);
        assert!(loaded.honesty_queue_ok(HostUpgradeKind::CaptureBuilding));

        // Completing via restored pending_index must work.
        loaded.record_complete(UPGRADE_INFANTRY_CAPTURE, 0, 11, 1);
        assert!(!loaded.honesty_queue_ok(HostUpgradeKind::CaptureBuilding));
        assert!(loaded.honesty_complete_ok(HostUpgradeKind::CaptureBuilding));
        assert_eq!(loaded.get(id).unwrap().units_affected, 1);
    }
}

// --- GLA Camouflage residual helpers (Upgrade_GLACamouflage / StealthUpgrade) ---

/// Retail GLA Camouflage upgrade name (Palace research).
pub const UPGRADE_GLA_CAMOUFLAGE: &str = "Upgrade_GLACamouflage";

/// Template names that receive StealthUpgrade from Camouflage residual.
///
/// C++: GLAInfantryRebel (+ Chem_/Demo_/CINE_ variants) StealthUpgrade
/// TriggeredBy = Upgrade_GLACamouflage. Fail-closed: not full StealthUpgrade
/// module on every general variant; workers do **not** receive Camouflage
/// (Worker has no StealthUpgrade — fail-closed honesty).
pub fn is_camouflage_unit_template(name: &str) -> bool {
    let n: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect();
    if n.is_empty() {
        return false;
    }
    // Explicit residual test names.
    if n == "testrebel" || n == "testglarebel" {
        return true;
    }
    // Rebel residual (not worker, not hijacker, not terrorist).
    if n.contains("worker") {
        return false;
    }
    n.contains("rebel") || n == "glainfantryrebel" || n.ends_with("infantryrebel")
}

/// C++ StealthUpdate StealthDelay residual for Rebel after Camouflage (2500 ms).
/// Host residual: re-cloak after this many logic frames once attack stops
/// (fail-closed vs full StealthUpdate forbidden-condition matrix).
pub const CAMOUFLAGE_STEALTH_DELAY_FRAMES: u32 = 75; // 2500ms @ 30 FPS

/// Retail Rebel Camouflage StealthForbiddenConditions residual tokens.
pub const CAMOUFLAGE_STEALTH_FORBIDDEN_CONDITIONS: &str = "ATTACKING USING_ABILITY";
/// Retail CamoNetting structure StealthForbiddenConditions residual tokens.
pub const CAMO_NETTING_STEALTH_FORBIDDEN_CONDITIONS: &str = "ATTACKING USING_ABILITY TAKING_DAMAGE";

/// Host residual of Camouflage infantry StealthUpdate::allowedToStealth.
///
/// Retail GLAInfantryRebel: StealthForbiddenConditions = ATTACKING USING_ABILITY.
/// Host residual also applies StealthDelay re-cloak gate after reveal.
pub fn camouflage_unit_stealth_desired(
    innate_stealth: bool,
    is_alive: bool,
    is_attacking: bool,
    is_using_ability: bool,
    current_frame: u32,
    stealth_allowed_frame: u32,
) -> Option<bool> {
    if !innate_stealth || !is_alive {
        return None;
    }
    if is_attacking || is_using_ability {
        return Some(false);
    }
    if stealth_allowed_frame > 0 && current_frame < stealth_allowed_frame {
        return Some(false);
    }
    Some(true)
}

/// Absolute frame when Camouflage residual may re-cloak after a reveal.
pub fn camouflage_stealth_allowed_frame(current_frame: u32) -> u32 {
    current_frame.saturating_add(CAMOUFLAGE_STEALTH_DELAY_FRAMES)
}

/// Whether a residual forbidden condition token is present in the retail string.
pub fn stealth_forbidden_contains(conditions: &str, token: &str) -> bool {
    conditions
        .split_whitespace()
        .any(|t| t.eq_ignore_ascii_case(token))
}

#[cfg(test)]
mod camouflage_template_tests {
    use super::*;

    #[test]
    fn camouflage_unit_name_residual() {
        assert!(is_camouflage_unit_template("GLAInfantryRebel"));
        assert!(is_camouflage_unit_template("Demo_GLAInfantryRebel"));
        assert!(is_camouflage_unit_template("Chem_GLAInfantryRebel"));
        assert!(is_camouflage_unit_template("TestRebel"));
        // Host shorthand GLA_Soldier is not a retail camouflage carrier (Rebel is).
        assert!(!is_camouflage_unit_template("GLA_Soldier"));
        assert!(!is_camouflage_unit_template("GLAInfantryWorker"));
        assert!(!is_camouflage_unit_template("GLAInfantryTerrorist"));
        assert!(!is_camouflage_unit_template("USA_Ranger"));
    }

    #[test]
    fn anthrax_toxin_kinds_from_name() {
        assert_eq!(
            HostUpgradeKind::from_name("Upgrade_GLAAnthraxBeta"),
            HostUpgradeKind::AnthraxBeta
        );
        assert_eq!(
            HostUpgradeKind::from_name("Upgrade_GLAToxinShells"),
            HostUpgradeKind::ToxinShells
        );
    }

    #[test]
    fn ap_bullets_kind_from_name() {
        assert_eq!(
            HostUpgradeKind::from_name("Upgrade_GLAAPBullets"),
            HostUpgradeKind::ApBullets
        );
        assert_eq!(
            HostUpgradeKind::from_name(UPGRADE_GLA_AP_BULLETS),
            HostUpgradeKind::ApBullets
        );
    }

    #[test]
    fn uranium_black_napalm_kinds_from_name() {
        assert_eq!(
            HostUpgradeKind::from_name("Upgrade_ChinaUraniumShells"),
            HostUpgradeKind::UraniumShells
        );
        assert_eq!(
            HostUpgradeKind::from_name("Upgrade_ChinaBlackNapalm"),
            HostUpgradeKind::BlackNapalm
        );
    }

    #[test]
    fn scorpion_laser_nationalism_chain_kinds_from_name() {
        assert_eq!(
            HostUpgradeKind::from_name("Upgrade_GLAScorpionRocket"),
            HostUpgradeKind::ScorpionRocket
        );
        assert_eq!(
            HostUpgradeKind::from_name("Upgrade_GLAAPRockets"),
            HostUpgradeKind::ApRockets
        );
        assert_eq!(
            HostUpgradeKind::from_name("Upgrade_AmericaLaserMissiles"),
            HostUpgradeKind::LaserMissiles
        );
        assert_eq!(
            HostUpgradeKind::from_name("Upgrade_ChinaNationalism"),
            HostUpgradeKind::Nationalism
        );
        assert_eq!(
            HostUpgradeKind::from_name("Upgrade_ChinaChainGuns"),
            HostUpgradeKind::ChainGuns
        );
    }

    #[test]
    fn subliminal_messaging_kind_from_name() {
        assert_eq!(
            HostUpgradeKind::from_name("Upgrade_ChinaSubliminalMessaging"),
            HostUpgradeKind::SubliminalMessaging
        );
        assert_eq!(
            HostUpgradeKind::from_name("Upgrade_ChinaSubliminalMessaging"),
            HostUpgradeKind::SubliminalMessaging
        );
    }

    #[test]
    fn advanced_control_rods_kind_from_name() {
        assert_eq!(
            HostUpgradeKind::from_name("Upgrade_AmericaAdvancedControlRods"),
            HostUpgradeKind::AdvancedControlRods
        );
        assert_eq!(
            HostUpgradeKind::from_name("SupW_Upgrade_AmericaAdvancedControlRods"),
            HostUpgradeKind::AdvancedControlRods
        );
        assert_eq!(
            HostUpgradeKind::from_name(UPGRADE_AMERICA_ADVANCED_CONTROL_RODS),
            HostUpgradeKind::AdvancedControlRods
        );
    }

    fn camouflage_kind_from_name() {
        assert_eq!(
            HostUpgradeKind::from_name("Upgrade_GLACamouflage"),
            HostUpgradeKind::Camouflage
        );
        assert_eq!(
            HostUpgradeKind::from_name("Upgrade_GLA_Camouflage"),
            HostUpgradeKind::Camouflage
        );
    }
}

// --- GLA CamoNetting residual helpers (Upgrade_GLACamoNetting / structure stealth) ---

/// Retail GLA CamoNetting upgrade name (Stealth General / Palace research residual).
pub const UPGRADE_GLA_CAMO_NETTING: &str = "Upgrade_GLACamoNetting";

/// Retail CamoNetting structure StealthDelay 2500ms → 75 frames @ 30 FPS.
///
/// Tunnel Network / Stinger Site / Stealth General buildings use StealthDelay
/// 2500 with StealthForbiddenConditions = ATTACKING USING_ABILITY TAKING_DAMAGE.
pub const CAMO_NETTING_STEALTH_DELAY_FRAMES: u32 = 75;

/// Retail CamoNetting FriendlyOpacityMin **50%** (StealthUpdate residual).
pub const CAMO_NETTING_FRIENDLY_OPACITY_MIN: f32 = 0.5;
/// Retail CamoNetting FriendlyOpacityMax **100%**.
pub const CAMO_NETTING_FRIENDLY_OPACITY_MAX: f32 = 1.0;
/// C++ StealthUpdate pulse phase rate residual (`m_pulsePhaseRate = 0.2`).
pub const CAMO_NETTING_OPACITY_PULSE_PHASE_RATE: f32 = 0.2;

// --- CamoNetting StealthLook / heat-vision residual (Drawable::setStealthLook) ---

/// C++ `StealthLookType` residual (Drawable::m_stealthLook ordinal parity).
///
/// Retail enum order (`Drawable.h`):
/// 0 NONE, 1 VISIBLE_FRIENDLY, 2 DISGUISED_ENEMY, 3 VISIBLE_DETECTED,
/// 4 VISIBLE_FRIENDLY_DETECTED, 5 INVISIBLE.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum HostCamoStealthLook {
    /// STEALTHLOOK_NONE — not stealthed.
    None = 0,
    /// STEALTHLOOK_VISIBLE_FRIENDLY — stealthed, friendly sees pulse opacity.
    VisibleFriendly = 1,
    /// STEALTHLOOK_DISGUISED_ENEMY — disguised (not full camo residual path).
    DisguisedEnemy = 2,
    /// STEALTHLOOK_VISIBLE_DETECTED — enemy detects stealthed unit (heat vision).
    VisibleDetected = 3,
    /// STEALTHLOOK_VISIBLE_FRIENDLY_DETECTED — friendly + detected (heat vision).
    VisibleFriendlyDetected = 4,
    /// STEALTHLOOK_INVISIBLE — stealthed, not detected by observer.
    Invisible = 5,
}

impl HostCamoStealthLook {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::VisibleFriendly,
            2 => Self::DisguisedEnemy,
            3 => Self::VisibleDetected,
            4 => Self::VisibleFriendlyDetected,
            5 => Self::Invisible,
            _ => Self::None,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Whether residual heat-vision second material pass is active.
    ///
    /// C++: VISIBLE_DETECTED / VISIBLE_FRIENDLY_DETECTED → secondMaterialPassOpacity=1
    /// (mines excluded — host residual structures never mines).
    pub fn heat_vision_active(self) -> bool {
        matches!(self, Self::VisibleFriendlyDetected | Self::VisibleDetected)
    }
}

/// Resolve CamoNetting structure StealthLook residual for an observer.
///
/// Host residual of StealthUpdate::calcStealthLook:
/// - not stealthed → None
/// - stealthed + same team (friendly) + !detected → VisibleFriendly
/// - stealthed + same team + detected → VisibleFriendlyDetected
/// - stealthed + enemy + detected → VisibleDetected
/// - stealthed + enemy + !detected → Invisible
///
/// Fail-closed: not full disguise / mine heat-vision hack / W3D second pass GPU.
pub fn camo_netting_stealth_look(
    stealthed: bool,
    detected: bool,
    observer_is_friendly: bool,
) -> HostCamoStealthLook {
    if !stealthed {
        return HostCamoStealthLook::None;
    }
    if observer_is_friendly {
        if detected {
            HostCamoStealthLook::VisibleFriendlyDetected
        } else {
            HostCamoStealthLook::VisibleFriendly
        }
    } else if detected {
        HostCamoStealthLook::VisibleDetected
    } else {
        HostCamoStealthLook::Invisible
    }
}

/// Heat-vision second material pass opacity residual (0.0 or 1.0).
pub fn camo_netting_heat_vision_opacity(look: HostCamoStealthLook) -> f32 {
    if look.heat_vision_active() {
        1.0
    } else {
        0.0
    }
}

/// Discrete FriendlyOpacity residual from cloaked / revealed state.
///
/// Stealthed-and-undetected → FriendlyOpacityMin; otherwise → Max.
pub fn camo_netting_friendly_opacity(stealthed: bool, detected: bool) -> f32 {
    if stealthed && !detected {
        CAMO_NETTING_FRIENDLY_OPACITY_MIN
    } else {
        CAMO_NETTING_FRIENDLY_OPACITY_MAX
    }
}

/// StealthUpdate pulse opacity residual while cloaked.
///
/// C++ drawable path: `0.5 + sin(phase) * 0.5` (range 0..1). Host residual maps
/// that factor into FriendlyOpacityMin..Max so cloaked structures pulse between
/// **50%** and **100%**. Returns `(opacity, next_phase)`.
pub fn camo_netting_pulse_opacity(phase: f32) -> (f32, f32) {
    let t = 0.5 + 0.5 * phase.sin(); // 0..1
    let opacity = CAMO_NETTING_FRIENDLY_OPACITY_MIN
        + (CAMO_NETTING_FRIENDLY_OPACITY_MAX - CAMO_NETTING_FRIENDLY_OPACITY_MIN) * t;
    let next_phase = phase + CAMO_NETTING_OPACITY_PULSE_PHASE_RATE;
    (opacity, next_phase)
}

// --- CamoNetting sub-object net mesh residual (presentation state, not GPU) ---

/// Residual honesty mesh name for structure camo-net sub-object presentation.
///
/// Retail CamoNetting is StealthUpgrade (not SubObjectsUpgrade), but drawable
/// presentation consumers still need a host residual "net mesh" show/hide +
/// opacity state analogous to a sub-object toggle. Fail-closed: not W3D mesh
/// GPU / SubObjectsUpgrade ShowSubObjects path.
pub const CAMO_NETTING_SUB_OBJECT_MESH_NAME: &str = "CamoNet";

/// Host residual of a CamoNetting structure net-mesh sub-object presentation.
///
/// Maps upgrade + StealthLook + FriendlyOpacity into a CPU-side descriptor
/// for presentation consumers. Fail-closed: not full W3D heat-vision GPU pass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostCamoNetSubObject {
    /// Residual mesh name honesty (`CamoNet`).
    pub mesh_name: &'static str,
    /// True when Upgrade_GLACamoNetting is applied (sub-object "shown").
    pub shown: bool,
    /// Effective opacity residual (FriendlyOpacity / pulse while cloaked).
    pub opacity: f32,
    /// Heat-vision second material pass residual active.
    pub heat_vision_pass: bool,
    /// StealthLook residual ordinal (host of Drawable::setStealthLook).
    pub stealth_look: HostCamoStealthLook,
}

/// Build CamoNetting sub-object net mesh residual presentation state.
///
/// - Upgrade not applied → shown=false (no net mesh residual)
/// - Upgrade applied → shown=true; opacity from FriendlyOpacity residual
/// - Detected residual → heat_vision_pass (second material pass opacity 1)
///
/// Fail-closed: not full W3D sub-object bone hide/show GPU matrix.
pub fn camo_netting_sub_object_state(
    upgrade_applied: bool,
    stealthed: bool,
    detected: bool,
    observer_is_friendly: bool,
    friendly_opacity: f32,
) -> HostCamoNetSubObject {
    let look = camo_netting_stealth_look(stealthed, detected, observer_is_friendly);
    let heat = camo_netting_heat_vision_opacity(look) > 0.5;
    let opacity = if upgrade_applied {
        friendly_opacity.clamp(
            CAMO_NETTING_FRIENDLY_OPACITY_MIN,
            CAMO_NETTING_FRIENDLY_OPACITY_MAX,
        )
    } else {
        CAMO_NETTING_FRIENDLY_OPACITY_MAX
    };
    HostCamoNetSubObject {
        mesh_name: CAMO_NETTING_SUB_OBJECT_MESH_NAME,
        shown: upgrade_applied,
        opacity,
        heat_vision_pass: upgrade_applied && heat,
        stealth_look: look,
    }
}

/// Whether residual net mesh should be presentation-visible to an observer.
///
/// Invisible StealthLook (enemy, undetected) → mesh residual hidden to that
/// observer. Friendly / detected residual → mesh can render with opacity.
pub fn camo_netting_sub_object_observer_visible(state: &HostCamoNetSubObject) -> bool {
    if !state.shown {
        return false;
    }
    !matches!(state.stealth_look, HostCamoStealthLook::Invisible)
}

/// Whether a CamoNetting residual structure should be stealthed this frame.
///
/// Host residual of StealthUpdate::allowedToStealth for structures:
/// - forbidden while attacking (ATTACKING / FIRING_PRIMARY residual)
/// - forbidden while using ability (USING_ABILITY residual)
/// - forbidden until StealthDelay after reveal (TAKING_DAMAGE / attack break)
/// - re-cloak when idle and delay elapsed (InnateStealth after StealthUpgrade)
///
/// Fail-closed: not full W3D sub-object net mesh GPU (host sub-object state closed).
pub fn camo_netting_structure_stealth_desired(
    innate_stealth: bool,
    is_alive: bool,
    is_attacking: bool,
    is_using_ability: bool,
    current_frame: u32,
    stealth_allowed_frame: u32,
) -> Option<bool> {
    if !innate_stealth || !is_alive {
        return None;
    }
    if is_attacking || is_using_ability {
        return Some(false);
    }
    if stealth_allowed_frame > 0 && current_frame < stealth_allowed_frame {
        return Some(false);
    }
    Some(true)
}

/// Absolute frame when CamoNetting residual may re-cloak after a reveal.
pub fn camo_netting_stealth_allowed_frame(current_frame: u32) -> u32 {
    current_frame.saturating_add(CAMO_NETTING_STEALTH_DELAY_FRAMES)
}

/// Whether an idle enemy residual unit should wake and attempt to target a
/// CamoNetting structure that just revealed (OrderIdleEnemiesToAttackMeUponReveal).
///
/// C++ `setWakeupIfInRange`: unit vision range ≥ distance to revealed victim.
/// Host residual: Idle AI + can_attack units (not structures/dozers) within vision.
pub fn camo_netting_order_idle_enemy_in_range(
    enemy_is_alive: bool,
    enemy_is_idle: bool,
    enemy_can_attack: bool,
    dist: f32,
    enemy_vision_range: f32,
) -> bool {
    enemy_is_alive
        && enemy_is_idle
        && enemy_can_attack
        && enemy_vision_range > 0.0
        && dist <= enemy_vision_range
}

/// Template names that receive StealthUpgrade from CamoNetting residual.
///
/// Retail: Stealth General buildings (Slth_*), GLATunnelNetwork, GLAStingerSite.
/// Fail-closed: not full sub-object camo net visual / every general reskin.
pub fn is_camo_netting_structure_template(name: &str) -> bool {
    let n: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect();
    if n.is_empty() {
        return false;
    }
    // Explicit residual tests.
    if n == "testcamonetstructure"
        || n == "testslthcommandcenter"
        || n == "testtunnelnetwork"
        || n == "teststingersite"
    {
        return true;
    }
    // Tunnel Network / Stinger Site residual (all general variants).
    if n.contains("tunnelnetwork") || n.contains("stingersite") {
        return true;
    }
    // Stealth General structure residual (Slth_ buildings).
    if n.starts_with("slth_") || n.starts_with("slth") || n.contains("gcslth") {
        // Building-like residual only (exclude infantry/vehicles).
        return n.contains("commandcenter")
            || n.contains("blackmarket")
            || n.contains("scudstorm")
            || n.contains("palace")
            || n.contains("supplystash")
            || n.contains("barracks")
            || n.contains("armsdealer")
            || n.contains("demotrap")
            || n.contains("fake");
    }
    false
}

// --- Anthrax Gamma residual helpers ---

/// Retail Chem Anthrax Gamma upgrade name.
pub const UPGRADE_CHEM_ANTHRAX_GAMMA: &str = "Chem_Upgrade_GLAAnthraxGamma";

/// Units that receive Anthrax Gamma residual combat tag on research complete.
pub fn is_anthrax_gamma_unit_template(name: &str) -> bool {
    use crate::game_logic::host_bomb_truck_detonate::is_bomb_truck_template;
    use crate::game_logic::host_scud_launcher::is_scud_launcher_template;
    use crate::game_logic::host_toxin_tractor::is_toxin_tractor_template;
    is_toxin_tractor_template(name)
        || is_scud_launcher_template(name)
        || is_bomb_truck_template(name)
}

// --- Wave 62 residual honesty packs (upgrade cost/time + stealth forbidden) ---

/// Retail Upgrade.ini BuildCost residual honesty pack.
pub fn honesty_upgrades_cost_residual_ok() -> bool {
    HostUpgradeKind::SupplyLines.retail_build_cost() == 800
        && HostUpgradeKind::FlashBangGrenade.retail_build_cost() == 800
        && HostUpgradeKind::TowMissile.retail_build_cost() == 800
        && HostUpgradeKind::CaptureBuilding.retail_build_cost() == 1000
        && HostUpgradeKind::CompositeArmor.retail_build_cost() == 2000
        && HostUpgradeKind::Camouflage.retail_build_cost() == 2000
        && HostUpgradeKind::CamoNetting.retail_build_cost() == 500
        && HostUpgradeKind::NeutronShells.retail_build_cost() == 2500
        && HostUpgradeKind::NuclearTanks.retail_build_cost() == 2000
        && HostUpgradeKind::BunkerBusters.retail_build_cost() == 1500
        && HostUpgradeKind::ComancheRocketPods.retail_build_cost() == 800
        && HostUpgradeKind::SentryDroneGun.retail_build_cost() == 1000
}

/// Retail Upgrade.ini BuildTime residual honesty pack (secs + frames).
pub fn honesty_upgrades_time_residual_ok() -> bool {
    (HostUpgradeKind::SupplyLines.retail_build_time_secs() - 30.0).abs() < 0.01
        && HostUpgradeKind::SupplyLines.retail_research_frames() == 900
        && (HostUpgradeKind::Camouflage.retail_build_time_secs() - 60.0).abs() < 0.01
        && HostUpgradeKind::Camouflage.retail_research_frames() == 1800
        && (HostUpgradeKind::CamoNetting.retail_build_time_secs() - 5.0).abs() < 0.01
        && HostUpgradeKind::CamoNetting.retail_research_frames() == 150
        && (HostUpgradeKind::NeutronShells.retail_build_time_secs() - 60.0).abs() < 0.01
        && HostUpgradeKind::NeutronShells.retail_research_frames() == 1800
        && (HostUpgradeKind::BunkerBusters.retail_build_time_secs() - 40.0).abs() < 0.01
        && HostUpgradeKind::BunkerBusters.retail_research_frames() == 1200
        // Host path remains short residual (observable queue).
        && HostUpgradeKind::SupplyLines.residual_research_frames() == 1
        && HostUpgradeKind::Camouflage.residual_research_frames() == 1
}

/// StealthForbiddenConditions residual honesty (Camouflage + CamoNetting).
pub fn honesty_upgrades_stealth_forbidden_residual_ok() -> bool {
    CAMOUFLAGE_STEALTH_FORBIDDEN_CONDITIONS == "ATTACKING USING_ABILITY"
        && stealth_forbidden_contains(CAMOUFLAGE_STEALTH_FORBIDDEN_CONDITIONS, "ATTACKING")
        && stealth_forbidden_contains(CAMOUFLAGE_STEALTH_FORBIDDEN_CONDITIONS, "USING_ABILITY")
        && !stealth_forbidden_contains(CAMOUFLAGE_STEALTH_FORBIDDEN_CONDITIONS, "TAKING_DAMAGE")
        && CAMO_NETTING_STEALTH_FORBIDDEN_CONDITIONS
            == "ATTACKING USING_ABILITY TAKING_DAMAGE"
        && stealth_forbidden_contains(CAMO_NETTING_STEALTH_FORBIDDEN_CONDITIONS, "TAKING_DAMAGE")
        && CAMOUFLAGE_STEALTH_DELAY_FRAMES == 75
        && CAMO_NETTING_STEALTH_DELAY_FRAMES == 75
        && camouflage_unit_stealth_desired(true, true, false, false, 100, 0) == Some(true)
        && camouflage_unit_stealth_desired(true, true, true, false, 100, 0) == Some(false)
        && camouflage_unit_stealth_desired(true, true, false, true, 100, 0) == Some(false)
        && camouflage_unit_stealth_desired(true, true, false, false, 50, 85) == Some(false)
        && camouflage_stealth_allowed_frame(10) == 85
        // CamoNetting structure forbidden residual already host-wired.
        && camo_netting_structure_stealth_desired(true, true, true, false, 100, 0) == Some(false)
        && camo_netting_structure_stealth_desired(true, true, false, true, 100, 0) == Some(false)
}

/// Combined Wave 62 upgrades residual honesty pack.
pub fn honesty_upgrades_residual_pack_ok() -> bool {
    honesty_upgrades_cost_residual_ok()
        && honesty_upgrades_time_residual_ok()
        && honesty_upgrades_stealth_forbidden_residual_ok()
}

/// Wave 79: retail cost/time residual **application** honesty (not docs-only).
///
/// Queue stamps `build_cost_paid` + `retail_research_frames` from Upgrade.ini
/// residual matrix; host research path remains 1-frame residual.
pub fn honesty_upgrades_cost_time_application_wave79_ok() -> bool {
    let mut reg = HostUpgradeRegistry::new();
    let id = reg.record_queue(UPGRADE_AMERICA_SUPPLY_LINES, Team::USA, 0, 10, None);
    let entry = reg.get(id).expect("queued");
    entry.build_cost_paid == HostUpgradeKind::SupplyLines.retail_build_cost()
        && entry.build_cost_paid == 800
        && entry.retail_research_frames == HostUpgradeKind::SupplyLines.retail_research_frames()
        && entry.retail_research_frames == 900
        && entry.residual_research_frames == 1
        && HostUpgradeKind::WorkerShoes.retail_build_cost() == 1000
        && HostUpgradeKind::CamoNetting.retail_build_cost() == 500
        && HostUpgradeKind::NuclearTanks.retail_research_frames() == 1800
        && resolve_upgrade_retail_cost_supplies("Upgrade_GLAWorkerShoes") == 1000
        && resolve_upgrade_retail_cost_supplies("Upgrade_AmericaSupplyLines") == 800
        && resolve_upgrade_retail_cost_supplies("Upgrade_AmericaAdvancedTraining") == 1500
        && honesty_upgrades_residual_pack_ok()
}

/// Wave 79 shared retail cost resolve (HostUpgradeKind matrix + AdvancedTraining).
///
/// Used by host residual command path honesty; fail-closed unknown → **0** for
/// known HostUpgradeKind::Other unless AdvancedTraining residual name matches.
pub fn resolve_upgrade_retail_cost_supplies(upgrade_name: &str) -> u32 {
    let kind = HostUpgradeKind::from_name(upgrade_name);
    let retail = kind.retail_build_cost();
    if retail > 0 {
        return retail;
    }
    let n = normalize_upgrade_identity(upgrade_name);
    if n.contains("advancedtraining") {
        return 1500; // Upgrade_AmericaAdvancedTraining BuildCost residual
    }
    0
}

#[cfg(test)]
mod camo_netting_and_gamma_tests {
    use super::*;

    #[test]
    fn camo_netting_structure_name_residual() {
        assert!(is_camo_netting_structure_template("Slth_GLACommandCenter"));
        assert!(is_camo_netting_structure_template("Slth_FakeGLABarracks"));
        assert!(is_camo_netting_structure_template("GLATunnelNetwork"));
        assert!(is_camo_netting_structure_template("Chem_GLATunnelNetwork"));
        assert!(is_camo_netting_structure_template("GLAStingerSite"));
        assert!(is_camo_netting_structure_template("TestCamoNetStructure"));
        assert!(!is_camo_netting_structure_template("GLAInfantryRebel"));
        assert!(!is_camo_netting_structure_template("AmericaCommandCenter"));
        assert!(!is_camo_netting_structure_template("Slth_GLAInfantryRebel"));
    }

    #[test]
    fn camo_netting_structure_stealth_delay_matrix() {
        assert_eq!(CAMO_NETTING_STEALTH_DELAY_FRAMES, 75);
        assert_eq!(camo_netting_stealth_allowed_frame(10), 85);

        // Idle + delay elapsed → recloak.
        assert_eq!(
            camo_netting_structure_stealth_desired(true, true, false, false, 100, 85),
            Some(true)
        );
        // Attacking residual forbids stealth.
        assert_eq!(
            camo_netting_structure_stealth_desired(true, true, true, false, 100, 0),
            Some(false)
        );
        // USING_ABILITY residual forbids stealth.
        assert_eq!(
            camo_netting_structure_stealth_desired(true, true, false, true, 100, 0),
            Some(false)
        );
        // Still inside StealthDelay after reveal.
        assert_eq!(
            camo_netting_structure_stealth_desired(true, true, false, false, 50, 85),
            Some(false)
        );
        // No CamoNetting residual (not innate).
        assert_eq!(
            camo_netting_structure_stealth_desired(false, true, false, false, 100, 0),
            None
        );
        // Dead structure.
        assert_eq!(
            camo_netting_structure_stealth_desired(true, false, false, false, 100, 0),
            None
        );
        // OrderIdleEnemies residual range gate.
        assert!(camo_netting_order_idle_enemy_in_range(
            true, true, true, 100.0, 150.0
        ));
        assert!(!camo_netting_order_idle_enemy_in_range(
            true, true, true, 200.0, 150.0
        ));
        assert!(!camo_netting_order_idle_enemy_in_range(
            true, false, true, 50.0, 150.0
        ));
        assert!(!camo_netting_order_idle_enemy_in_range(
            true, true, false, 50.0, 150.0
        ));

        // FriendlyOpacity residual matrix.
        assert!((CAMO_NETTING_FRIENDLY_OPACITY_MIN - 0.5).abs() < 0.001);
        assert!((CAMO_NETTING_FRIENDLY_OPACITY_MAX - 1.0).abs() < 0.001);
        assert!((camo_netting_friendly_opacity(true, false) - 0.5).abs() < 0.001);
        assert!((camo_netting_friendly_opacity(true, true) - 1.0).abs() < 0.001);
        assert!((camo_netting_friendly_opacity(false, false) - 1.0).abs() < 0.001);
        // Pulse residual: phase 0 → mid-span opacity 0.75; phase advances by 0.2.
        let (op0, ph1) = camo_netting_pulse_opacity(0.0);
        assert!((op0 - 0.75).abs() < 0.001);
        assert!((ph1 - 0.2).abs() < 0.001);
        let (op_pi_2, _) = camo_netting_pulse_opacity(std::f32::consts::FRAC_PI_2);
        assert!((op_pi_2 - 1.0).abs() < 0.001);
        let (op_3pi_2, _) = camo_netting_pulse_opacity(3.0 * std::f32::consts::FRAC_PI_2);
        assert!((op_3pi_2 - 0.5).abs() < 0.001);

        // StealthLook / heat-vision residual matrix (C++ Drawable.h ordinals).
        assert_eq!(HostCamoStealthLook::None.as_u8(), 0);
        assert_eq!(HostCamoStealthLook::VisibleFriendly.as_u8(), 1);
        assert_eq!(HostCamoStealthLook::DisguisedEnemy.as_u8(), 2);
        assert_eq!(HostCamoStealthLook::VisibleDetected.as_u8(), 3);
        assert_eq!(HostCamoStealthLook::VisibleFriendlyDetected.as_u8(), 4);
        assert_eq!(HostCamoStealthLook::Invisible.as_u8(), 5);
        assert_eq!(
            HostCamoStealthLook::from_u8(3),
            HostCamoStealthLook::VisibleDetected
        );
        assert_eq!(
            HostCamoStealthLook::from_u8(4),
            HostCamoStealthLook::VisibleFriendlyDetected
        );
        assert_eq!(
            HostCamoStealthLook::from_u8(5),
            HostCamoStealthLook::Invisible
        );
        assert_eq!(
            camo_netting_stealth_look(false, false, true),
            HostCamoStealthLook::None
        );
        assert_eq!(
            camo_netting_stealth_look(true, false, true),
            HostCamoStealthLook::VisibleFriendly
        );
        assert_eq!(
            camo_netting_stealth_look(true, true, true),
            HostCamoStealthLook::VisibleFriendlyDetected
        );
        assert_eq!(
            camo_netting_stealth_look(true, true, false),
            HostCamoStealthLook::VisibleDetected
        );
        assert_eq!(
            camo_netting_stealth_look(true, false, false),
            HostCamoStealthLook::Invisible
        );
        assert!(!HostCamoStealthLook::VisibleFriendly.heat_vision_active());
        assert!(HostCamoStealthLook::VisibleDetected.heat_vision_active());
        assert!(HostCamoStealthLook::VisibleFriendlyDetected.heat_vision_active());
        assert!(
            (camo_netting_heat_vision_opacity(HostCamoStealthLook::VisibleDetected) - 1.0).abs()
                < 0.001
        );
        assert!(
            (camo_netting_heat_vision_opacity(HostCamoStealthLook::Invisible) - 0.0).abs() < 0.001
        );

        // CamoNetting sub-object net mesh residual (presentation, not GPU).
        assert_eq!(CAMO_NETTING_SUB_OBJECT_MESH_NAME, "CamoNet");
        let no_upg = camo_netting_sub_object_state(false, false, false, false, 1.0);
        assert!(!no_upg.shown);
        assert!(!camo_netting_sub_object_observer_visible(&no_upg));
        let cloaked = camo_netting_sub_object_state(
            true,
            true,
            false,
            true,
            CAMO_NETTING_FRIENDLY_OPACITY_MIN,
        );
        assert!(cloaked.shown);
        assert_eq!(cloaked.mesh_name, "CamoNet");
        assert!((cloaked.opacity - 0.5).abs() < 0.001);
        assert!(!cloaked.heat_vision_pass);
        assert_eq!(cloaked.stealth_look, HostCamoStealthLook::VisibleFriendly);
        assert!(camo_netting_sub_object_observer_visible(&cloaked));
        let detected = camo_netting_sub_object_state(true, true, true, false, 1.0);
        assert!(detected.shown);
        assert!(detected.heat_vision_pass);
        assert_eq!(detected.stealth_look, HostCamoStealthLook::VisibleDetected);
        assert!(camo_netting_sub_object_observer_visible(&detected));
        let invisible = camo_netting_sub_object_state(true, true, false, false, 0.5);
        assert!(invisible.shown);
        assert_eq!(invisible.stealth_look, HostCamoStealthLook::Invisible);
        assert!(!camo_netting_sub_object_observer_visible(&invisible));
    }

    #[test]
    fn camo_netting_and_gamma_kind_from_name() {
        assert_eq!(
            HostUpgradeKind::from_name("Upgrade_GLACamoNetting"),
            HostUpgradeKind::CamoNetting
        );
        assert_eq!(
            HostUpgradeKind::from_name("Chem_Upgrade_GLAAnthraxGamma"),
            HostUpgradeKind::AnthraxGamma
        );
        assert_eq!(
            HostUpgradeKind::from_name("Upgrade_GLAAnthraxGamma"),
            HostUpgradeKind::AnthraxGamma
        );
        assert_eq!(
            HostUpgradeKind::from_name("Demo_Upgrade_SuicideBomb"),
            HostUpgradeKind::SuicideBomb
        );
        // Camouflage still distinct.
        assert_eq!(
            HostUpgradeKind::from_name("Upgrade_GLACamouflage"),
            HostUpgradeKind::Camouflage
        );
    }

    #[test]
    fn upgrades_residual_pack_honesty() {
        assert!(honesty_upgrades_cost_residual_ok());
        assert!(honesty_upgrades_time_residual_ok());
        assert!(honesty_upgrades_stealth_forbidden_residual_ok());
        assert!(honesty_upgrades_residual_pack_ok());
    }

    /// Wave 72 residual pack honesty gate (wrapper residual_pack_ok).
    #[test]
    fn upgrades_residual_pack_honesty_wave72() {
        assert!(honesty_upgrades_residual_pack_ok());
        assert!(honesty_upgrades_cost_residual_ok());
        assert!(honesty_upgrades_time_residual_ok());
        assert!(honesty_upgrades_stealth_forbidden_residual_ok());
        assert_eq!(HostUpgradeKind::SupplyLines.retail_build_cost(), 800);
        assert_eq!(HostUpgradeKind::Camouflage.retail_research_frames(), 1800);
        assert_eq!(CAMOUFLAGE_STEALTH_DELAY_FRAMES, 75);
    }

    /// Wave 79: retail cost/time residual application honesty.
    #[test]
    fn upgrades_cost_time_application_wave79_honesty() {
        assert!(honesty_upgrades_cost_time_application_wave79_ok());
        let mut reg = HostUpgradeRegistry::new();
        let id = reg.record_queue(UPGRADE_GLA_WORKER_SHOES, Team::GLA, 1, 0, None);
        reg.set_build_cost_paid(UPGRADE_GLA_WORKER_SHOES, 1, 1000);
        let e = reg.get(id).unwrap();
        assert_eq!(e.build_cost_paid, 1000);
        assert_eq!(e.retail_research_frames, 300); // 10s * 30
        assert_eq!(e.residual_research_frames, 1);
    }
}
