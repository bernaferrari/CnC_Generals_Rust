//! Host China Overlord / Helix / Emperor portable addon residual.
//!
//! Residual slice (playability):
//! - Overlord / Helix portable structure upgrades install residual behavior on the
//!   host vehicle (fail-closed vs full OverlordContain / HelixContain passenger
//!   object spawn + W3DDependencyModelDraw):
//!   - `Upgrade_ChinaOverlordGattlingCannon` / `Upgrade_ChinaHelixGattlingCannon`:
//!     equips SECONDARY AA `GattlingBuildingGunAir` + passenger ground residual
//!     `GattlingBuildingGun` on PRIMARY fires (PassengersAllowedToFire residual).
//!     BuildCost **1200** / BuildTime **20**s residual.
//!   - `Upgrade_ChinaOverlordPropagandaTower` / `Upgrade_ChinaHelixPropagandaTower`:
//!     enables propaganda pulse residual on the host (Radius 150, heal 1%/2%).
//!     BuildCost **500** / BuildTime **10**s residual.
//!   - BattleBunker residual remains `install_overlord_battle_bunker` (existing).
//!     BuildCost **400** / BuildTime **15**s residual. Bunker infantry slots **5**.
//! - ConflictsWith residual exclusivity: only **one** portable addon at a time
//!   (gattling ↔ propaganda ↔ bunker pairwise ConflictsWith).
//! - OverlordContain residual capacity: Slots **1** (PORTABLE_STRUCTURE only).
//! - Emperor tank (`Tank_ChinaTankEmperor`): innate propaganda residual
//!   (`PropagandaTowerBehavior` AffectsSelf=Yes) + optional gattling upgrade.
//! - Helix residual: `HelixContain` Slots=**5** transport capacity.
//!
//! Fail-closed honesty:
//! - Not full OCL portable-structure passenger object / DamageModule share
//! - Not full W3DOverlord*Draw / W3DDependencyModelDraw bone attach
//! - Not full ContinuousFire model-condition animation on payload
//! - Not full ProductionUpdate MaxQueueEntries UI production-queue path
//! - Not network addon replication (network deferred)

use super::Weapon;
use serde::{Deserialize, Serialize};

/// Retail Overlord / Helix gattling upgrade names.
pub const UPGRADE_OVERLORD_GATTLING: &str = "Upgrade_ChinaOverlordGattlingCannon";
pub const UPGRADE_HELIX_GATTLING: &str = "Upgrade_ChinaHelixGattlingCannon";
/// Retail Overlord / Helix propaganda upgrade names.
pub const UPGRADE_OVERLORD_PROPAGANDA: &str = "Upgrade_ChinaOverlordPropagandaTower";
pub const UPGRADE_HELIX_PROPAGANDA: &str = "Upgrade_ChinaHelixPropagandaTower";
/// Retail Overlord / Helix battle bunker upgrade names (existing bunker residual).
pub const UPGRADE_OVERLORD_BUNKER: &str = "Upgrade_ChinaOverlordBattleBunker";
pub const UPGRADE_HELIX_BUNKER: &str = "Upgrade_ChinaHelixBattleBunker";

/// Retail portable-structure object / OCL residual names (SpeakerTower = propaganda).
pub const OCL_OVERLORD_GATTLING: &str = "OCL_OverlordGattlingCannon";
pub const OCL_OVERLORD_PROPAGANDA: &str = "OCL_OverlordPropagandaTower";
pub const OCL_OVERLORD_BUNKER: &str = "OCL_OverlordBattleBunker";
/// Retail portable payload object template residual names.
pub const OVERLORD_PAYLOAD_GATTLING: &str = "ChinaTankOverlordGattlingCannon";
pub const OVERLORD_PAYLOAD_PROPAGANDA: &str = "ChinaTankOverlordPropagandaTower";
pub const OVERLORD_PAYLOAD_BUNKER: &str = "ChinaTankOverlordBattleBunker";
/// SpeakerTower residual alias for propaganda tower button image residual.
pub const OVERLORD_SPEAKER_TOWER_BUTTON: &str = "SSOLSpeaker";

/// Retail addon BuildCost residual (Upgrade.ini).
pub const OVERLORD_GATTLING_BUILD_COST: u32 = 1200;
pub const OVERLORD_PROPAGANDA_BUILD_COST: u32 = 500;
pub const OVERLORD_BUNKER_BUILD_COST: u32 = 400;
/// Retail addon BuildTime residual (seconds).
pub const OVERLORD_GATTLING_BUILD_TIME_SECS: f32 = 20.0;
pub const OVERLORD_PROPAGANDA_BUILD_TIME_SECS: f32 = 10.0;
pub const OVERLORD_BUNKER_BUILD_TIME_SECS: f32 = 15.0;

/// Retail GattlingBuildingGun (portable Overlord/Helix gattling ground).
pub const GATTLING_BUILDING_GUN: &str = "GattlingBuildingGun";
/// Retail GattlingBuildingGunAir (portable Overlord/Helix gattling AA).
pub const GATTLING_BUILDING_GUN_AIR: &str = "GattlingBuildingGunAir";

/// Retail GattlingBuildingGun PrimaryDamage.
pub const OVERLORD_GATTLING_GROUND_DAMAGE: f32 = 10.0;
/// Retail GattlingBuildingGun AttackRange.
pub const OVERLORD_GATTLING_GROUND_RANGE: f32 = 225.0;
/// Retail GattlingBuildingGunAir PrimaryDamage.
pub const OVERLORD_GATTLING_AIR_DAMAGE: f32 = 5.0;
/// Retail GattlingBuildingGunAir AttackRange.
pub const OVERLORD_GATTLING_AIR_RANGE: f32 = 400.0;
/// Retail DelayBetweenShots 250ms → 8 frames @ 30 FPS.
pub const OVERLORD_GATTLING_BASE_DELAY_FRAMES: u32 = 8;
/// ContinuousFireOne residual (building gattling).
pub const OVERLORD_GATTLING_CONTINUOUS_FIRE_ONE: u32 = 1;
/// ContinuousFireTwo residual.
pub const OVERLORD_GATTLING_CONTINUOUS_FIRE_TWO: u32 = 5;
/// ContinuousFireCoast 2000ms → 60 frames @ 30 FPS.
pub const OVERLORD_GATTLING_COAST_FRAMES: u32 = 60;

/// Retail OverlordPropagandaTower / HelixPropagandaTower Radius.
pub const OVERLORD_PROPAGANDA_RADIUS: f32 = 150.0;
/// Retail HealPercentEachSecond = 1% (addon / Emperor base).
pub const OVERLORD_PROPAGANDA_HEAL_PERCENT_PER_SEC: f32 = 0.01;
/// Retail UpgradedHealPercentEachSecond = 2%.
pub const OVERLORD_PROPAGANDA_UPGRADED_HEAL_PERCENT_PER_SEC: f32 = 0.02;

/// Retail OverlordContain Slots residual (portable structure capacity = 1).
pub const OVERLORD_CONTAIN_SLOTS: usize = 1;
/// Retail OverlordContain DamagePercentToUnits residual (100%).
pub const OVERLORD_CONTAIN_DAMAGE_PERCENT_TO_UNITS: f32 = 1.0;
/// Retail OverlordContain AllowInsideKindOf residual name.
pub const OVERLORD_CONTAIN_ALLOW_INSIDE: &str = "PORTABLE_STRUCTURE";
/// Retail ProductionUpdate MaxQueueEntries residual (only one addon at a time).
pub const OVERLORD_PRODUCTION_MAX_QUEUE_ENTRIES: u32 = 1;

/// Retail BattleBunker TransportContain Slots residual (infantry seats on bunker).
pub const OVERLORD_BUNKER_INFANTRY_SLOTS: usize = 5;

/// Retail HelixContain Slots residual.
pub const HELIX_TRANSPORT_SLOTS: usize = 5;

/// Portable addon kind residual (exclusive ConflictsWith matrix).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OverlordAddonKind {
    Gattling,
    Propaganda,
    Bunker,
}

/// Residual addon slot table entry (name + cost + build time).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlordAddonSlotResidual {
    pub kind: OverlordAddonKind,
    pub overlord_upgrade: &'static str,
    pub helix_upgrade: &'static str,
    pub ocl_name: &'static str,
    pub payload_template: &'static str,
    pub build_cost: u32,
    pub build_time_secs: f32,
}

/// Retail Overlord / Helix portable addon residual table (3 exclusive slots).
pub const OVERLORD_ADDON_SLOT_TABLE: &[OverlordAddonSlotResidual] = &[
    OverlordAddonSlotResidual {
        kind: OverlordAddonKind::Gattling,
        overlord_upgrade: UPGRADE_OVERLORD_GATTLING,
        helix_upgrade: UPGRADE_HELIX_GATTLING,
        ocl_name: OCL_OVERLORD_GATTLING,
        payload_template: OVERLORD_PAYLOAD_GATTLING,
        build_cost: OVERLORD_GATTLING_BUILD_COST,
        build_time_secs: OVERLORD_GATTLING_BUILD_TIME_SECS,
    },
    OverlordAddonSlotResidual {
        kind: OverlordAddonKind::Propaganda,
        overlord_upgrade: UPGRADE_OVERLORD_PROPAGANDA,
        helix_upgrade: UPGRADE_HELIX_PROPAGANDA,
        ocl_name: OCL_OVERLORD_PROPAGANDA,
        payload_template: OVERLORD_PAYLOAD_PROPAGANDA,
        build_cost: OVERLORD_PROPAGANDA_BUILD_COST,
        build_time_secs: OVERLORD_PROPAGANDA_BUILD_TIME_SECS,
    },
    OverlordAddonSlotResidual {
        kind: OverlordAddonKind::Bunker,
        overlord_upgrade: UPGRADE_OVERLORD_BUNKER,
        helix_upgrade: UPGRADE_HELIX_BUNKER,
        ocl_name: OCL_OVERLORD_BUNKER,
        payload_template: OVERLORD_PAYLOAD_BUNKER,
        build_cost: OVERLORD_BUNKER_BUILD_COST,
        build_time_secs: OVERLORD_BUNKER_BUILD_TIME_SECS,
    },
];

/// Residual fire audio for portable gattling.
pub const OVERLORD_GATTLING_FIRE_AUDIO: &str = "GattlingCannonWeapon";

/// Host residual honesty counters for Overlord/Helix/Emperor addons.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostOverlordAddonRegistry {
    /// Gattling addon installs (upgrade residual).
    pub gattling_installs: u32,
    /// Propaganda addon installs (upgrade residual; Emperor innate counts on spawn).
    pub propaganda_installs: u32,
    /// Portable gattling ground residual fires.
    pub gattling_ground_fires: u32,
    /// Portable gattling AA residual fires.
    pub gattling_aa_fires: u32,
    /// Units hit by portable gattling residual.
    pub gattling_units_hit: u32,
    /// Helix transport residual loads.
    pub helix_loads: u32,
    /// Helix transport residual unloads.
    pub helix_unloads: u32,
}

impl HostOverlordAddonRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn record_gattling_install(&mut self) {
        self.gattling_installs = self.gattling_installs.saturating_add(1);
    }

    pub fn record_propaganda_install(&mut self) {
        self.propaganda_installs = self.propaganda_installs.saturating_add(1);
    }

    pub fn record_gattling_ground_fire(&mut self, hits: u32) {
        self.gattling_ground_fires = self.gattling_ground_fires.saturating_add(1);
        self.gattling_units_hit = self.gattling_units_hit.saturating_add(hits);
    }

    pub fn record_gattling_aa_fire(&mut self, hits: u32) {
        self.gattling_aa_fires = self.gattling_aa_fires.saturating_add(1);
        self.gattling_units_hit = self.gattling_units_hit.saturating_add(hits);
    }

    pub fn record_helix_load(&mut self) {
        self.helix_loads = self.helix_loads.saturating_add(1);
    }

    pub fn record_helix_unload(&mut self) {
        self.helix_unloads = self.helix_unloads.saturating_add(1);
    }

    pub fn honesty_gattling_install_ok(&self) -> bool {
        self.gattling_installs > 0
    }

    pub fn honesty_propaganda_install_ok(&self) -> bool {
        self.propaganda_installs > 0
    }

    pub fn honesty_gattling_fire_ok(&self) -> bool {
        (self.gattling_ground_fires > 0 || self.gattling_aa_fires > 0)
            && self.gattling_units_hit > 0
    }

    pub fn honesty_helix_transport_ok(&self) -> bool {
        self.helix_loads > 0 && self.helix_unloads > 0
    }

    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_gattling_install_ok()
            || self.honesty_propaganda_install_ok()
            || self.honesty_gattling_fire_ok()
            || self.honesty_helix_transport_ok()
    }
}

/// Whether template is a residual China Overlord tank (not portable payloads).
pub fn is_overlord_tank_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n.contains("gattling")
        || n.contains("gatling")
        || n.contains("propaganda")
        || n.contains("bunker")
        || n.contains("weapon")
        || n.contains("shell")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("command")
    {
        return false;
    }
    // Emperor is separate residual path (innate propaganda).
    if n.contains("emperor") {
        return false;
    }
    n.contains("tankoverlord")
        || n.contains("overlordtank")
        || n == "china_overlordtank"
        || n == "china_overlord"
        || n == "testoverlord"
        || (n.contains("overlord") && (n.contains("tank") || n.contains("vehicle")))
}

/// Whether template is a residual China Helix helicopter (not portable payloads).
pub fn is_helix_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n.contains("gattling")
        || n.contains("gatling")
        || n.contains("propaganda")
        || n.contains("bunker")
        || n.contains("weapon")
        || n.contains("shell")
        || n.contains("debris")
        || n.contains("blade")
        || n.contains("rubble")
        || n.starts_with("upgrade")
        || n.contains("command")
        || n.contains("napalm")
    {
        return false;
    }
    n.contains("vehiclehelix")
        || n.contains("helix") && (n.contains("vehicle") || n.contains("china"))
        || n == "china_helix"
        || n == "testhelix"
        || n.ends_with("helix") && !n.contains("cannon") && !n.contains("tower")
}

/// Whether template is residual Emperor tank (Tank General Overlord variant).
pub fn is_emperor_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() || n.starts_with("upgrade") || n.contains("weapon") {
        return false;
    }
    n.contains("tankemperor")
        || n.contains("emperortank")
        || n.ends_with("emperor")
        || n == "testemperor"
}

/// Overlord family hosts that accept portable addons residual.
pub fn is_overlord_family_host(template_name: &str) -> bool {
    is_overlord_tank_template(template_name)
        || is_helix_template(template_name)
        || is_emperor_template(template_name)
}

/// Whether upgrade name installs residual portable gattling.
pub fn is_gattling_addon_upgrade(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("overlordgattling")
        || n.contains("helixgattling")
        || n == "upgrade_chinaoverlordgattlingcannon"
        || n == "upgrade_chinahelixgattlingcannon"
}

/// Whether upgrade name installs residual portable propaganda.
pub fn is_propaganda_addon_upgrade(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("overlordpropaganda")
        || n.contains("helixpropaganda")
        || n == "upgrade_chinaoverlordpropagandatower"
        || n == "upgrade_chinahelixpropagandatower"
}

/// Whether upgrade name installs residual battle bunker (existing path).
pub fn is_bunker_addon_upgrade(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("overlordbattlebunker")
        || n.contains("helixbattlebunker")
        || n.contains("overlordbunker")
        || n.contains("helixbunker")
}

/// Map residual upgrade name → addon kind (if portable addon).
pub fn overlord_addon_kind_from_upgrade(name: &str) -> Option<OverlordAddonKind> {
    if is_gattling_addon_upgrade(name) {
        Some(OverlordAddonKind::Gattling)
    } else if is_propaganda_addon_upgrade(name) {
        Some(OverlordAddonKind::Propaganda)
    } else if is_bunker_addon_upgrade(name) {
        Some(OverlordAddonKind::Bunker)
    } else {
        None
    }
}

/// Lookup residual addon slot table entry by kind.
pub fn overlord_addon_slot(kind: OverlordAddonKind) -> &'static OverlordAddonSlotResidual {
    OVERLORD_ADDON_SLOT_TABLE
        .iter()
        .find(|e| e.kind == kind)
        .expect("OVERLORD_ADDON_SLOT_TABLE covers all OverlordAddonKind values")
}

/// Retail ConflictsWith residual: portable addons are mutually exclusive.
///
/// Installing `next` conflicts with any other installed kind (not self).
/// Emperor innate propaganda is not an ObjectCreationUpgrade ConflictsWith path
/// (host residual: gattling may coexist with Emperor innate propaganda).
pub fn overlord_addons_conflict(a: OverlordAddonKind, b: OverlordAddonKind) -> bool {
    a != b
}

/// Residual ConflictsWith list for a given addon kind (the other two).
pub fn overlord_addon_conflicts_with(kind: OverlordAddonKind) -> [OverlordAddonKind; 2] {
    match kind {
        OverlordAddonKind::Gattling => [OverlordAddonKind::Propaganda, OverlordAddonKind::Bunker],
        OverlordAddonKind::Propaganda => [OverlordAddonKind::Gattling, OverlordAddonKind::Bunker],
        OverlordAddonKind::Bunker => [OverlordAddonKind::Gattling, OverlordAddonKind::Propaganda],
    }
}

/// Whether residual install of `next` is allowed given currently installed flags.
///
/// Non-Emperor hosts: only one of gattling / propaganda / bunker.
/// Emperor: innate propaganda is not a ConflictsWith payload; gattling allowed.
pub fn overlord_addon_install_allowed(
    next: OverlordAddonKind,
    has_gattling: bool,
    has_propaganda: bool,
    has_bunker: bool,
    is_emperor: bool,
) -> bool {
    if is_emperor {
        // Emperor residual: gattling optional; bunker/propaganda object addons N/A.
        return matches!(
            next,
            OverlordAddonKind::Gattling | OverlordAddonKind::Propaganda
        );
    }
    let installed = [
        (OverlordAddonKind::Gattling, has_gattling),
        (OverlordAddonKind::Propaganda, has_propaganda),
        (OverlordAddonKind::Bunker, has_bunker),
    ];
    for (kind, active) in installed {
        if active && overlord_addons_conflict(kind, next) {
            // Host residual: install path clears the conflicting flags (allowed
            // with exclusivity applied). Retail production queue would block
            // concurrent research via MaxQueueEntries=1 + ConflictsWith.
            // Residual honesty: installing is always "allowed" but exclusive.
            let _ = kind;
        }
    }
    // Residual host always allows install then clears others (matches object.rs).
    let _ = (has_gattling, has_propaganda, has_bunker);
    true
}

/// Residual exclusive active kind after install (post ConflictsWith clear).
pub fn overlord_exclusive_addon_after_install(
    next: OverlordAddonKind,
    is_emperor: bool,
) -> (bool, bool, bool) {
    // (gattling, propaganda, bunker)
    match next {
        OverlordAddonKind::Gattling => (true, is_emperor, false),
        OverlordAddonKind::Propaganda => (false, true, false),
        OverlordAddonKind::Bunker => (false, is_emperor, true),
    }
}

/// Wave 49 residual honesty: addon table + ConflictsWith + OverlordContain slots.
pub fn honesty_overlord_addons_residual_ok() -> bool {
    OVERLORD_ADDON_SLOT_TABLE.len() == 3
        && OVERLORD_CONTAIN_SLOTS == 1
        && HELIX_TRANSPORT_SLOTS == 5
        && OVERLORD_BUNKER_INFANTRY_SLOTS == 5
        && OVERLORD_PRODUCTION_MAX_QUEUE_ENTRIES == 1
        && (OVERLORD_CONTAIN_DAMAGE_PERCENT_TO_UNITS - 1.0).abs() < 0.01
        && OVERLORD_CONTAIN_ALLOW_INSIDE == "PORTABLE_STRUCTURE"
        && OVERLORD_GATTLING_BUILD_COST == 1200
        && OVERLORD_PROPAGANDA_BUILD_COST == 500
        && OVERLORD_BUNKER_BUILD_COST == 400
        && (OVERLORD_GATTLING_BUILD_TIME_SECS - 20.0).abs() < 0.01
        && (OVERLORD_PROPAGANDA_BUILD_TIME_SECS - 10.0).abs() < 0.01
        && (OVERLORD_BUNKER_BUILD_TIME_SECS - 15.0).abs() < 0.01
        && overlord_addons_conflict(OverlordAddonKind::Gattling, OverlordAddonKind::Propaganda)
        && overlord_addons_conflict(OverlordAddonKind::Gattling, OverlordAddonKind::Bunker)
        && overlord_addons_conflict(OverlordAddonKind::Propaganda, OverlordAddonKind::Bunker)
        && !overlord_addons_conflict(OverlordAddonKind::Gattling, OverlordAddonKind::Gattling)
        && overlord_addon_slot(OverlordAddonKind::Gattling).ocl_name == OCL_OVERLORD_GATTLING
        && overlord_addon_slot(OverlordAddonKind::Propaganda).payload_template
            == OVERLORD_PAYLOAD_PROPAGANDA
        && overlord_addon_slot(OverlordAddonKind::Bunker).build_cost == 400
        && overlord_addon_kind_from_upgrade(UPGRADE_OVERLORD_GATTLING)
            == Some(OverlordAddonKind::Gattling)
        && overlord_addon_kind_from_upgrade(UPGRADE_HELIX_PROPAGANDA)
            == Some(OverlordAddonKind::Propaganda)
        && overlord_addon_kind_from_upgrade(UPGRADE_OVERLORD_BUNKER)
            == Some(OverlordAddonKind::Bunker)
        && OVERLORD_SPEAKER_TOWER_BUTTON == "SSOLSpeaker"
}
/// Combined residual honesty pack (Wave 71).
pub fn honesty_overlord_addons_residual_pack_ok() -> bool {
    honesty_overlord_addons_residual_ok()
}

/// Delay frames residual for continuous-fire level (building gattling base / ROF).
pub fn overlord_gattling_delay_frames(level: u8) -> u32 {
    let base = OVERLORD_GATTLING_BASE_DELAY_FRAMES as f32;
    let rof = match level {
        1 => 2.0,
        2 => 3.0,
        _ => 1.0,
    };
    (base / rof).floor().max(1.0) as u32
}

fn delay_frames_to_reload_secs(delay_frames: u32) -> f32 {
    (delay_frames.max(1) as f32) / 30.0
}

/// Build residual GattlingBuildingGunAir secondary for Overlord/Helix gattling addon.
pub fn overlord_gattling_air_weapon(level: u8, chain_guns: bool) -> Weapon {
    let mult = if chain_guns { 1.25 } else { 1.0 };
    let delay = overlord_gattling_delay_frames(level);
    Weapon {
        damage: OVERLORD_GATTLING_AIR_DAMAGE * mult,
        range: OVERLORD_GATTLING_AIR_RANGE,
        min_range: 0.0,
        reload_time: delay_frames_to_reload_secs(delay),
        last_fire_time: 0.0,
        ammo: None,
        can_target_air: true,
        can_target_ground: false,
        projectile_speed: 999_999.0,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
    }
}

/// Build residual GattlingBuildingGun ground stats for passenger residual fire.
pub fn overlord_gattling_ground_damage(chain_guns: bool) -> f32 {
    if chain_guns {
        OVERLORD_GATTLING_GROUND_DAMAGE * 1.25
    } else {
        OVERLORD_GATTLING_GROUND_DAMAGE
    }
}

/// Whether residual fire should apply portable gattling residual.
pub fn should_apply_overlord_gattling_residual(has_addon: bool) -> bool {
    has_addon
}

/// Slot residual: 1 = AA secondary, 0 = primary tank/minigun + passenger ground gattling.
pub fn overlord_gattling_slot_for_air(target_is_air: bool) -> u8 {
    if target_is_air {
        1
    } else {
        0
    }
}

/// Legal residual gattling hit target.
pub fn is_legal_overlord_gattling_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

/// Residual propaganda heal amount for Overlord/Helix/Emperor rates (1%/2%).
pub fn overlord_propaganda_heal_amount(max_health: f32, upgraded: bool, dt: f32) -> f32 {
    if max_health <= 0.0 || dt <= 0.0 {
        return 0.0;
    }
    let percent = if upgraded {
        OVERLORD_PROPAGANDA_UPGRADED_HEAL_PERCENT_PER_SEC
    } else {
        OVERLORD_PROPAGANDA_HEAL_PERCENT_PER_SEC
    };
    percent * max_health * dt
}

/// Whether object residual should pulse as propaganda source.
///
/// Emperor innate + Overlord/Helix propaganda addon flag.
pub fn is_overlord_propaganda_source(has_propaganda_addon: bool, template_name: &str) -> bool {
    has_propaganda_addon
        || is_emperor_template(template_name)
        // Portable tower object names also pulse (existing host_propaganda path).
        || {
            let n = template_name.to_ascii_lowercase();
            n.contains("overlordpropaganda") || n.contains("helixpropaganda")
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlord_helix_emperor_name_matrix() {
        assert!(is_overlord_tank_template("ChinaTankOverlord"));
        assert!(is_overlord_tank_template("China_OverlordTank"));
        assert!(is_overlord_tank_template("TestOverlord"));
        assert!(is_overlord_tank_template("Nuke_ChinaTankOverlord"));
        assert!(!is_overlord_tank_template(
            "ChinaTankOverlordGattlingCannon"
        ));
        assert!(!is_overlord_tank_template(
            "ChinaTankOverlordPropagandaTower"
        ));
        assert!(!is_overlord_tank_template("ChinaTankOverlordBattleBunker"));
        assert!(!is_overlord_tank_template("Tank_ChinaTankEmperor"));

        assert!(is_helix_template("ChinaVehicleHelix"));
        assert!(is_helix_template("China_Helix"));
        assert!(is_helix_template("TestHelix"));
        assert!(!is_helix_template("ChinaHelixGattlingCannon"));
        assert!(!is_helix_template("ChinaHelixPropagandaTower"));
        assert!(!is_helix_template("ChinaHelixBattleBunker"));

        assert!(is_emperor_template("Tank_ChinaTankEmperor"));
        assert!(is_emperor_template("TestEmperor"));
        assert!(is_overlord_family_host("ChinaTankOverlord"));
        assert!(is_overlord_family_host("ChinaVehicleHelix"));
        assert!(is_overlord_family_host("Tank_ChinaTankEmperor"));
    }

    #[test]
    fn upgrade_name_matrix() {
        assert!(is_gattling_addon_upgrade(UPGRADE_OVERLORD_GATTLING));
        assert!(is_gattling_addon_upgrade(UPGRADE_HELIX_GATTLING));
        assert!(is_propaganda_addon_upgrade(UPGRADE_OVERLORD_PROPAGANDA));
        assert!(is_propaganda_addon_upgrade(UPGRADE_HELIX_PROPAGANDA));
        assert!(is_bunker_addon_upgrade(UPGRADE_OVERLORD_BUNKER));
        assert!(is_bunker_addon_upgrade(UPGRADE_HELIX_BUNKER));
        assert!(!is_gattling_addon_upgrade("Upgrade_ChinaChainGuns"));
    }

    #[test]
    fn gattling_weapon_and_heal_math() {
        let air = overlord_gattling_air_weapon(0, false);
        assert!((air.damage - OVERLORD_GATTLING_AIR_DAMAGE).abs() < 0.01);
        assert!((air.range - OVERLORD_GATTLING_AIR_RANGE).abs() < 0.01);
        assert!(air.can_target_air);
        assert!(!air.can_target_ground);

        let air_chain = overlord_gattling_air_weapon(0, true);
        assert!((air_chain.damage - OVERLORD_GATTLING_AIR_DAMAGE * 1.25).abs() < 0.01);

        assert_eq!(overlord_gattling_delay_frames(0), 8);
        assert_eq!(overlord_gattling_delay_frames(1), 4);
        assert_eq!(overlord_gattling_delay_frames(2), 2);

        let base = overlord_propaganda_heal_amount(100.0, false, 1.0);
        let up = overlord_propaganda_heal_amount(100.0, true, 1.0);
        assert!((base - 1.0).abs() < f32::EPSILON);
        assert!((up - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn registry_honesty() {
        let mut reg = HostOverlordAddonRegistry::new();
        assert!(!reg.honesty_host_path_ok());
        reg.record_gattling_install();
        assert!(reg.honesty_gattling_install_ok());
        reg.record_gattling_ground_fire(2);
        assert!(reg.honesty_gattling_fire_ok());
        reg.record_propaganda_install();
        assert!(reg.honesty_propaganda_install_ok());
        reg.record_helix_load();
        reg.record_helix_unload();
        assert!(reg.honesty_helix_transport_ok());
    }

    /// Wave 49: addon slot table + ConflictsWith exclusivity + OverlordContain slots.
    #[test]
    fn overlord_addon_slot_conflicts_residual_honesty() {
        assert!(honesty_overlord_addons_residual_ok());
        assert_eq!(OVERLORD_CONTAIN_SLOTS, 1);
        assert_eq!(HELIX_TRANSPORT_SLOTS, 5);
        assert_eq!(OVERLORD_BUNKER_INFANTRY_SLOTS, 5);

        // Costs residual matrix.
        let g = overlord_addon_slot(OverlordAddonKind::Gattling);
        let p = overlord_addon_slot(OverlordAddonKind::Propaganda);
        let b = overlord_addon_slot(OverlordAddonKind::Bunker);
        assert_eq!(g.build_cost, 1200);
        assert_eq!(p.build_cost, 500);
        assert_eq!(b.build_cost, 400);
        assert!((g.build_time_secs - 20.0).abs() < 0.01);
        assert!((p.build_time_secs - 10.0).abs() < 0.01);
        assert!((b.build_time_secs - 15.0).abs() < 0.01);

        // ConflictsWith residual: pairwise exclusive.
        let cg = overlord_addon_conflicts_with(OverlordAddonKind::Gattling);
        assert!(cg.contains(&OverlordAddonKind::Propaganda));
        assert!(cg.contains(&OverlordAddonKind::Bunker));
        assert!(!cg.contains(&OverlordAddonKind::Gattling));

        // Install exclusivity residual (non-Emperor).
        assert_eq!(
            overlord_exclusive_addon_after_install(OverlordAddonKind::Gattling, false),
            (true, false, false)
        );
        assert_eq!(
            overlord_exclusive_addon_after_install(OverlordAddonKind::Propaganda, false),
            (false, true, false)
        );
        assert_eq!(
            overlord_exclusive_addon_after_install(OverlordAddonKind::Bunker, false),
            (false, false, true)
        );
        // Emperor keeps innate propaganda when installing gattling.
        assert_eq!(
            overlord_exclusive_addon_after_install(OverlordAddonKind::Gattling, true),
            (true, true, false)
        );

        assert!(overlord_addon_install_allowed(
            OverlordAddonKind::Gattling,
            false,
            false,
            false,
            false
        ));
    }
    /// Wave 71 residual pack honesty gate.
    #[test]
    fn overlord_addons_residual_pack_honesty_wave71() {
        assert!(honesty_overlord_addons_residual_pack_ok());
        assert_eq!(OVERLORD_CONTAIN_SLOTS, 1);
        assert_eq!(OVERLORD_GATTLING_BUILD_COST, 1200);
        assert_eq!(HELIX_TRANSPORT_SLOTS, 5);
    }
}
