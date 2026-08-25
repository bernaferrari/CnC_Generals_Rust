//! PlayerTemplate + PlayerTemplateStore
//!
//! C++ oracle:
//! - `GeneralsMD/Code/GameEngine/Include/Common/PlayerTemplate.h`
//! - `GeneralsMD/Code/GameEngine/Source/Common/RTS/PlayerTemplate.cpp`
//!
//! INI field parse lives in `crate::common::ini::ini_player_template` and must not be
//! rewritten here. This module owns the runtime type, store APIs, and C++-named
//! accessors used after parse.

use crate::common::game_common::VeterancyLevel;
use crate::common::name_key_generator::{NAMEKEY_INVALID, NameKeyGenerator, NameKeyType};
use crate::common::rts::Money;
use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

/// C++ `MAX_MP_STARTING_UNITS`
pub const MAX_MP_STARTING_UNITS: usize = 10;

/// Player template defining faction characteristics (`class PlayerTemplate`).
#[derive(Debug, Clone)]
pub struct PlayerTemplate {
    /// C++ `m_nameKey`
    pub name_key: NameKeyType,
    /// Cached `KEYNAME(m_nameKey)` (C++ `getName()`).
    pub name: String,
    pub display_name: String,
    pub side: String,
    pub base_side: String,
    /// C++ `m_playableSide`. Default `false` matches `PlayerTemplate::PlayerTemplate()`.
    pub playable: bool,
    /// C++ `m_observer`
    pub is_observer: bool,
    /// C++ `m_oldFaction`
    pub old_faction: bool,
    /// C++ `m_money`
    pub starting_money: Money,
    /// Packed 0xRRGGBB from INI `PreferredColor` (C++ `RGBColor m_preferredColor`).
    pub preferred_color: u32,
    pub starting_building: String,
    /// C++ `m_startingUnits[MAX_MP_STARTING_UNITS]`
    pub starting_units: Vec<String>,
    pub intrinsic_sciences: Vec<String>,
    pub purchase_science_command_set_rank1: String,
    pub purchase_science_command_set_rank3: String,
    pub purchase_science_command_set_rank8: String,
    pub special_power_shortcut_command_set: String,
    pub special_power_shortcut_win_name: String,
    pub special_power_shortcut_button_count: i32,
    pub intrinsic_science_purchase_points: i32,
    pub score_screen_image: String,
    pub load_screen_image: String,
    pub load_screen_music: String,
    pub score_screen_music: String,
    pub head_water_mark: String,
    pub flag_water_mark: String,
    pub enabled_image: String,
    pub side_icon_image: String,
    pub general_image: String,
    pub beacon_name: String,
    pub army_tooltip: String,
    pub features: String,
    pub medallion_regular: String,
    pub medallion_hilite: String,
    pub medallion_select: String,
    pub production_cost_changes: HashMap<NameKeyType, f32>,
    pub production_time_changes: HashMap<NameKeyType, f32>,
    pub production_veterancy_levels: HashMap<NameKeyType, VeterancyLevel>,
    /// Not present on C++ `PlayerTemplate`; retained for GameLogic side-dict seeding.
    pub player_allies: String,
    /// Not present on C++ `PlayerTemplate`; retained for GameLogic side-dict seeding.
    pub player_enemies: String,
}

impl PlayerTemplate {
    pub fn new(name: String) -> Self {
        // C++ constructs with `NAMEKEY_INVALID` then `setNameKey` after `initFromINI`.
        // `parse_player_template_definition` never calls `set_name_key`, so we bind the
        // namekey here (same end-state as C++ after parse).
        let name_key = if name.is_empty() {
            NAMEKEY_INVALID
        } else {
            NameKeyGenerator::name_to_key(&name)
        };
        Self {
            name_key,
            name,
            display_name: String::new(),
            side: String::new(),
            base_side: String::new(),
            playable: false,
            is_observer: false,
            old_faction: false,
            starting_money: Money::new(),
            preferred_color: 0,
            starting_building: String::new(),
            starting_units: vec![String::new(); MAX_MP_STARTING_UNITS],
            intrinsic_sciences: Vec::new(),
            purchase_science_command_set_rank1: String::new(),
            purchase_science_command_set_rank3: String::new(),
            purchase_science_command_set_rank8: String::new(),
            special_power_shortcut_command_set: String::new(),
            special_power_shortcut_win_name: String::new(),
            special_power_shortcut_button_count: 0,
            intrinsic_science_purchase_points: 0,
            score_screen_image: String::new(),
            load_screen_image: String::new(),
            load_screen_music: String::new(),
            score_screen_music: String::new(),
            head_water_mark: String::new(),
            flag_water_mark: String::new(),
            enabled_image: String::new(),
            side_icon_image: String::new(),
            general_image: String::new(),
            beacon_name: String::new(),
            army_tooltip: String::new(),
            features: String::new(),
            medallion_regular: String::new(),
            medallion_hilite: String::new(),
            medallion_select: String::new(),
            production_cost_changes: HashMap::new(),
            production_time_changes: HashMap::new(),
            production_veterancy_levels: HashMap::new(),
            player_allies: String::new(),
            player_enemies: String::new(),
        }
    }

    /// C++ `setNameKey`
    pub fn set_name_key(&mut self, name_key: NameKeyType) {
        self.name_key = name_key;
        if let Some(name) = NameKeyGenerator::key_to_name(name_key) {
            self.name = name;
        }
    }

    /// C++ `getNameKey` (asserts non-invalid in debug).
    pub fn get_name_key(&self) -> NameKeyType {
        debug_assert_ne!(self.name_key, NAMEKEY_INVALID, "bad namekey");
        self.name_key
    }

    /// C++ `getName()` → `KEYNAME(m_nameKey)`
    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_display_name(&self) -> &str {
        // C++ returns `m_displayName` even when empty. Keep a name fallback for
        // existing UI/callers that treat an empty label as "use the template name".
        if self.display_name.is_empty() {
            &self.name
        } else {
            &self.display_name
        }
    }

    /// C++ `getSide()`
    pub fn get_side(&self) -> &str {
        &self.side
    }

    /// C++ `getBaseSide()`
    pub fn get_base_side(&self) -> &str {
        &self.base_side
    }

    pub fn get_side_icon_image(&self) -> &str {
        &self.side_icon_image
    }

    /// C++ `getStartingBuilding()`
    pub fn get_starting_building(&self) -> &str {
        &self.starting_building
    }

    /// C++ `getStartingUnit(Int i)` — empty if `i < 0` or `i >= MAX_MP_STARTING_UNITS`.
    pub fn get_starting_unit(&self, i: i32) -> &str {
        if i < 0 || (i as usize) >= MAX_MP_STARTING_UNITS {
            return "";
        }
        self.starting_units
            .get(i as usize)
            .map(String::as_str)
            .unwrap_or("")
    }

    /// C++ `getMoney()`
    pub fn get_money(&self) -> &Money {
        &self.starting_money
    }

    /// C++ `getPreferredColor()` — packed 0xRRGGBB (INI stores RGBColor as u32).
    pub fn get_preferred_color(&self) -> u32 {
        self.preferred_color
    }

    /// C++ `isObserver()` → `m_observer`
    pub fn is_observer(&self) -> bool {
        self.is_observer
    }

    /// C++ `isPlayableSide()` → `m_playableSide` only.
    ///
    /// Zero Hour UI/ladder call sites additionally skip `side == "Boss"`
    /// (`WOLWelcomeMenu.cpp`, `LadderDefs.cpp`). That filter is **not** part of
    /// this method; see [`is_playable_side_excluding_boss`].
    pub fn is_playable_side(&self) -> bool {
        self.playable
    }

    /// ZH shell/ladder filter: playable and not the hidden Boss side.
    ///
    /// C++ applies this at the call site, not on `PlayerTemplate::isPlayableSide`.
    pub fn is_playable_side_excluding_boss(&self) -> bool {
        self.is_playable_side() && self.side != "Boss"
    }

    /// C++ `isOldFaction()`
    pub fn is_old_faction(&self) -> bool {
        self.old_faction
    }

    /// C++ `getProductionCostChanges()`
    pub fn get_production_cost_changes(&self) -> &HashMap<NameKeyType, f32> {
        &self.production_cost_changes
    }

    /// C++ `getProductionTimeChanges()`
    pub fn get_production_time_changes(&self) -> &HashMap<NameKeyType, f32> {
        &self.production_time_changes
    }

    /// C++ `getProductionVeterancyLevels()`
    pub fn get_production_veterancy_levels(&self) -> &HashMap<NameKeyType, VeterancyLevel> {
        &self.production_veterancy_levels
    }

    /// C++ `getScoreScreen()`
    pub fn get_score_screen(&self) -> &str {
        &self.score_screen_image
    }

    /// C++ `getLoadScreen()`
    pub fn get_load_screen(&self) -> &str {
        &self.load_screen_image
    }

    /// C++ `getBeaconTemplate()`
    pub fn get_beacon_template(&self) -> &str {
        &self.beacon_name
    }

    /// C++ `getTooltip()`
    pub fn get_tooltip(&self) -> &str {
        &self.army_tooltip
    }

    /// C++ `getGeneralFeatures()`
    pub fn get_general_features(&self) -> &str {
        &self.features
    }

    /// C++ `getIntrinsicSciencePurchasePoints()`
    pub fn get_intrinsic_science_purchase_points(&self) -> i32 {
        self.intrinsic_science_purchase_points
    }

    /// C++ `getIntrinsicSciences()`. Common keeps INI names; GameLogic resolves
    /// them to `ScienceType` (namekeys) in `from_common`.
    pub fn get_intrinsic_sciences(&self) -> &[String] {
        &self.intrinsic_sciences
    }

    /// C++ `getPurchaseScienceCommandSetRank1()`
    pub fn get_purchase_science_command_set_rank1(&self) -> &str {
        &self.purchase_science_command_set_rank1
    }

    /// C++ `getPurchaseScienceCommandSetRank3()`
    pub fn get_purchase_science_command_set_rank3(&self) -> &str {
        &self.purchase_science_command_set_rank3
    }

    /// C++ `getPurchaseScienceCommandSetRank8()`
    pub fn get_purchase_science_command_set_rank8(&self) -> &str {
        &self.purchase_science_command_set_rank8
    }

    /// C++ `getSpecialPowerShortcutCommandSet()`
    pub fn get_special_power_shortcut_command_set(&self) -> &str {
        &self.special_power_shortcut_command_set
    }

    /// C++ `getSpecialPowerShortcutWinName()`
    pub fn get_special_power_shortcut_win_name(&self) -> &str {
        &self.special_power_shortcut_win_name
    }

    /// C++ `getSpecialPowerShortcutButtonCount()`
    pub fn get_special_power_shortcut_button_count(&self) -> i32 {
        self.special_power_shortcut_button_count
    }

    /// C++ `getLoadScreenMusic()`
    pub fn get_load_screen_music(&self) -> &str {
        &self.load_screen_music
    }

    /// C++ `getScoreScreenMusic()`
    pub fn get_score_screen_music(&self) -> &str {
        &self.score_screen_music
    }

    /// C++ `getMedallionNormal()`
    pub fn get_medallion_normal(&self) -> &str {
        &self.medallion_regular
    }

    /// C++ `getMedallionHilite()`
    pub fn get_medallion_hilite(&self) -> &str {
        &self.medallion_hilite
    }

    /// C++ `getMedallionSelected()`
    pub fn get_medallion_selected(&self) -> &str {
        &self.medallion_select
    }

    /// Image-name accessor matching C++ `getHeadWaterMarkImage` (name only).
    pub fn get_head_water_mark(&self) -> &str {
        &self.head_water_mark
    }

    /// Image-name accessor matching C++ `getFlagWaterMarkImage` (name only).
    pub fn get_flag_water_mark(&self) -> &str {
        &self.flag_water_mark
    }

    /// Image-name accessor matching C++ `getEnabledImage` (name only).
    pub fn get_enabled_image(&self) -> &str {
        &self.enabled_image
    }

    /// Image-name accessor matching C++ `getGeneralImage` (name only).
    pub fn get_general_image(&self) -> &str {
        &self.general_image
    }
}

/// Remap obsolete Choose-A-General / subfaction namekeys to the base ZH factions.
/// C++ `PlayerTemplateStore::findPlayerTemplate` ("ugly, hokey code to quietly load old maps").
fn remap_legacy_faction_namekey(namekey: NameKeyType) -> NameKeyType {
    let america = NameKeyGenerator::name_to_key("FactionAmerica");
    let china = NameKeyGenerator::name_to_key("FactionChina");
    let gla = NameKeyGenerator::name_to_key("FactionGLA");

    let america_old = [
        NameKeyGenerator::name_to_key("FactionAmericaChooseAGeneral"),
        NameKeyGenerator::name_to_key("FactionAmericaTankCommand"),
        NameKeyGenerator::name_to_key("FactionAmericaSpecialForces"),
        NameKeyGenerator::name_to_key("FactionAmericaAirForce"),
    ];
    let china_old = [
        NameKeyGenerator::name_to_key("FactionChinaChooseAGeneral"),
        NameKeyGenerator::name_to_key("FactionChinaRedArmy"),
        NameKeyGenerator::name_to_key("FactionChinaSpecialWeapons"),
        NameKeyGenerator::name_to_key("FactionChinaSecretPolice"),
    ];
    let gla_old = [
        NameKeyGenerator::name_to_key("FactionGLAChooseAGeneral"),
        NameKeyGenerator::name_to_key("FactionGLATerrorCell"),
        NameKeyGenerator::name_to_key("FactionGLABiowarCommand"),
        NameKeyGenerator::name_to_key("FactionGLAWarlordCommand"),
    ];

    if america_old.contains(&namekey) {
        america
    } else if china_old.contains(&namekey) {
        china
    } else if gla_old.contains(&namekey) {
        gla
    } else {
        namekey
    }
}

/// Player template store (`class PlayerTemplateStore`).
#[derive(Debug)]
pub struct PlayerTemplateStore {
    templates: Vec<PlayerTemplate>,
}

impl PlayerTemplateStore {
    pub fn new() -> Self {
        Self {
            templates: Vec::new(),
        }
    }

    /// C++ `PlayerTemplateStore::init` — clears the list.
    pub fn init(&mut self) {
        self.templates.clear();
    }

    /// C++ `PlayerTemplateStore::reset` — intentionally retains templates.
    pub fn reset(&mut self) {
        // don't reset this list here; we want to retain this info.
    }

    /// C++ `PlayerTemplateStore::update` — no-op.
    pub fn update(&mut self) {}

    /// String lookup used by existing callers / tests. Exact name match (no remapping).
    pub fn find_template(&self, name: &str) -> Option<&PlayerTemplate> {
        self.templates.iter().find(|t| t.name == name)
    }

    /// C++ `findPlayerTemplate(NameKeyType)` including old-map faction remapping.
    pub fn find_player_template(&self, namekey: NameKeyType) -> Option<&PlayerTemplate> {
        let namekey = remap_legacy_faction_namekey(namekey);
        self.templates.iter().find(|t| t.name_key == namekey)
    }

    /// C++ `getNthPlayerTemplate(Int i)` — `None` when out of range.
    pub fn get_nth_player_template(&self, index: usize) -> Option<&PlayerTemplate> {
        self.templates.get(index)
    }

    /// Signed-index variant matching C++ `Int i` (`i < 0` → `None`).
    pub fn get_nth_player_template_signed(&self, i: i32) -> Option<&PlayerTemplate> {
        usize::try_from(i)
            .ok()
            .and_then(|index| self.get_nth_player_template(index))
    }

    pub fn get_nth_player_template_mut(&mut self, index: usize) -> Option<&mut PlayerTemplate> {
        self.templates.get_mut(index)
    }

    pub fn add_template(&mut self, template: PlayerTemplate) {
        self.templates.push(template);
    }

    /// Find-or-create index used by `parse_player_template_definition`.
    /// Matches C++ parse: locate by namekey (without old-map remapping, so INI
    /// definitions such as `FactionAmericaAirForceGeneral` stay distinct).
    pub fn find_template_index(&self, name: &str) -> Option<usize> {
        let key = NameKeyGenerator::name_to_key(name);
        self.templates
            .iter()
            .position(|template| template.name_key == key || template.name == name)
    }

    /// C++ `getPlayerTemplateCount()`
    pub fn get_player_template_count(&self) -> i32 {
        self.templates.len() as i32
    }

    pub fn len(&self) -> usize {
        self.templates.len()
    }

    pub fn is_empty(&self) -> bool {
        self.templates.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, PlayerTemplate> {
        self.templates.iter()
    }

    pub fn clear(&mut self) {
        self.templates.clear();
    }

    /// C++ `getTemplateNumByName` — case-insensitive, `-1` if missing.
    pub fn get_template_num_by_name(&self, name: &str) -> i32 {
        self.templates
            .iter()
            .position(|template| template.get_name().eq_ignore_ascii_case(name))
            .map(|i| i as i32)
            .unwrap_or(-1)
    }

    /// C++ `getAllSideStrings`: unique sides in first-seen order, appended to `out`.
    pub fn get_all_side_strings(&self, out: &mut Vec<String>) {
        let mut tmp: Vec<String> = Vec::new();
        for i in 0..self.get_player_template_count() {
            let Some(pt) = self.get_nth_player_template_signed(i) else {
                continue;
            };
            if !tmp.iter().any(|side| side == pt.get_side()) {
                tmp.push(pt.get_side().to_string());
            }
        }
        out.extend(tmp);
    }
}

impl Default for PlayerTemplateStore {
    fn default() -> Self {
        Self::new()
    }
}

static PLAYER_TEMPLATE_STORE: OnceCell<RwLock<PlayerTemplateStore>> = OnceCell::new();

pub fn get_player_template_store() -> RwLockReadGuard<'static, PlayerTemplateStore> {
    PLAYER_TEMPLATE_STORE
        .get_or_init(|| RwLock::new(PlayerTemplateStore::new()))
        .read()
        .expect("PlayerTemplateStore poisoned")
}

/// Non-blocking read so map-load player sync can fail-open if INI load holds the write lock.
pub fn try_get_player_template_store() -> Option<RwLockReadGuard<'static, PlayerTemplateStore>> {
    PLAYER_TEMPLATE_STORE
        .get_or_init(|| RwLock::new(PlayerTemplateStore::new()))
        .try_read()
        .ok()
}

pub fn get_player_template_store_mut() -> RwLockWriteGuard<'static, PlayerTemplateStore> {
    PLAYER_TEMPLATE_STORE
        .get_or_init(|| RwLock::new(PlayerTemplateStore::new()))
        .write()
        .expect("PlayerTemplateStore poisoned")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::ini::INI;
    use crate::common::language::Language;

    fn reset_player_template_test_state() {
        Language::clear_localized_strings();
        get_player_template_store_mut().clear();
    }

    #[test]
    fn get_starting_unit_bounds_empty_without_ini() {
        let pt = PlayerTemplate::new("FactionTest".into());
        assert_eq!(pt.get_starting_unit(-1), "");
        assert_eq!(pt.get_starting_unit(0), "");
        assert_eq!(pt.get_starting_unit(9), "");
        assert_eq!(pt.get_starting_unit(10), "");
        assert_eq!(pt.get_starting_unit(MAX_MP_STARTING_UNITS as i32), "");
    }

    #[test]
    fn is_playable_side_matches_cpp_playable_flag_only() {
        let mut pt = PlayerTemplate::new("FactionBoss".into());
        pt.playable = true;
        pt.side = "Boss".into();
        assert!(pt.is_playable_side());
        assert!(!pt.is_playable_side_excluding_boss());

        pt.playable = false;
        assert!(!pt.is_playable_side());
    }

    #[test]
    fn player_template_ini_parse_starting_units_money_production_and_store() {
        reset_player_template_test_state();

        let mut ini = INI::new();
        ini.with_inline_source(
            r#"
PlayerTemplate FactionAmerica
  Side = America
  BaseSide = America
  PlayableSide = Yes
  StartMoney = 10000
  PreferredColor = R:0 G:0 B:255
  StartingBuilding = AmericaCommandCenter
  StartingUnit0 = AmericaRanger
  StartingUnit1 = AmericaDozer
  ProductionCostChange = AmericaTank 80%
  ProductionTimeChange = AmericaTank 50%
  ProductionVeterancyLevel = AmericaRanger VETERAN
  IsObserver = No
  OldFaction = Yes
End

PlayerTemplate FactionChina
  Side = China
  PlayableSide = Yes
  StartMoney = 8000
  StartingUnit0 = ChinaRedguard
  IsObserver = No
  OldFaction = Yes
End

PlayerTemplate FactionGLA
  Side = GLA
  PlayableSide = Yes
End

PlayerTemplate FactionAmericaAirForce
  Side = America
  PlayableSide = Yes
End

PlayerTemplate FactionObserver
  Side = Observer
  PlayableSide = No
  IsObserver = Yes
  OldFaction = No
End

PlayerTemplate FactionBoss
  Side = Boss
  PlayableSide = Yes
  IsObserver = No
End
"#,
            |ini| ini.parse_current_file(),
        )
        .expect("inline PlayerTemplate should parse");

        {
            let store = get_player_template_store();
            assert_eq!(store.get_player_template_count(), 6);

            let america = store
                .find_template("FactionAmerica")
                .expect("FactionAmerica stored");

            assert_eq!(america.get_starting_unit(0), "AmericaRanger");
            assert_eq!(america.get_starting_unit(1), "AmericaDozer");
            assert_eq!(america.get_starting_unit(9), "");
            assert_eq!(america.get_starting_unit(-1), "");
            assert_eq!(america.get_starting_unit(10), "");
            assert_eq!(america.get_starting_building(), "AmericaCommandCenter");

            assert_eq!(america.get_money().count_money(), 10000);
            assert_eq!(america.get_preferred_color(), 0x0000FF);

            let tank_key = NameKeyGenerator::name_to_key("AmericaTank");
            let ranger_key = NameKeyGenerator::name_to_key("AmericaRanger");
            assert_eq!(
                america
                    .get_production_cost_changes()
                    .get(&tank_key)
                    .copied(),
                Some(0.8)
            );
            assert_eq!(
                america
                    .get_production_time_changes()
                    .get(&tank_key)
                    .copied(),
                Some(0.5)
            );
            assert_eq!(
                america
                    .get_production_veterancy_levels()
                    .get(&ranger_key)
                    .copied(),
                Some(VeterancyLevel::Veteran)
            );

            assert!(america.is_playable_side());
            assert!(!america.is_observer());
            assert!(america.is_old_faction());

            let observer = store
                .find_template("FactionObserver")
                .expect("FactionObserver stored");
            assert!(observer.is_observer());
            assert!(!observer.is_playable_side());
            assert!(!observer.is_old_faction());

            let boss = store
                .find_template("FactionBoss")
                .expect("FactionBoss stored");
            assert!(boss.is_playable_side());
            assert!(!boss.is_playable_side_excluding_boss());

            let america_key = NameKeyGenerator::name_to_key("FactionAmerica");
            let found = store
                .find_player_template(america_key)
                .expect("find by namekey");
            assert_eq!(found.get_name_key(), america.get_name_key());
            assert_eq!(found.get_name(), "FactionAmerica");

            let remapped = store
                .find_player_template(NameKeyGenerator::name_to_key("FactionAmericaTankCommand"))
                .expect("old America namekey remaps");
            assert_eq!(remapped.get_name(), "FactionAmerica");

            assert_eq!(store.get_template_num_by_name("factionamerica"), 0);
            assert_eq!(store.get_template_num_by_name("FactionChina"), 1);
            assert_eq!(store.get_template_num_by_name("missing"), -1);

            assert!(store.get_nth_player_template_signed(-1).is_none());
            assert!(store.get_nth_player_template(0).is_some());
            assert!(store.get_nth_player_template(99).is_none());

            let mut sides = Vec::new();
            store.get_all_side_strings(&mut sides);
            assert_eq!(sides, vec!["America", "China", "GLA", "Observer", "Boss"]);

            // C++ splices onto the existing list rather than replacing it.
            store.get_all_side_strings(&mut sides);
            assert_eq!(
                sides,
                vec![
                    "America", "China", "GLA", "Observer", "Boss", "America", "China", "GLA",
                    "Observer", "Boss"
                ]
            );
        }

        reset_player_template_test_state();
    }

    #[test]
    fn parse_player_template_definition_finds_or_creates_by_namekey() {
        reset_player_template_test_state();

        let mut ini = INI::new();
        ini.with_inline_source(
            r#"
PlayerTemplate FactionAmerica
  Side = America
  StartMoney = 1000
  StartingUnit0 = AmericaRanger
End

PlayerTemplate FactionAmerica
  Side = America
  StartMoney = 2500
  StartingUnit0 = AmericaMissileDefender
  StartingUnit1 = AmericaDozer
End
"#,
            |ini| ini.parse_current_file(),
        )
        .expect("redefinition should parse");

        {
            let store = get_player_template_store();
            assert_eq!(store.get_player_template_count(), 1);
            let america = store
                .find_player_template(NameKeyGenerator::name_to_key("FactionAmerica"))
                .expect("namekey find after redefinition");
            assert_eq!(america.get_money().count_money(), 2500);
            assert_eq!(america.get_starting_unit(0), "AmericaMissileDefender");
            assert_eq!(america.get_starting_unit(1), "AmericaDozer");
        }

        reset_player_template_test_state();
    }

    #[test]
    fn store_init_clears_but_reset_retains() {
        reset_player_template_test_state();
        {
            let mut store = get_player_template_store_mut();
            store.add_template(PlayerTemplate::new("FactionAmerica".into()));
            store.reset();
            assert_eq!(store.get_player_template_count(), 1);
            store.init();
            assert_eq!(store.get_player_template_count(), 0);
        }
        reset_player_template_test_state();
    }
}
