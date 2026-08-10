use super::*;

/// Player relation map (matching C++ PlayerRelationMap)
pub type PlayerRelationMapType = HashMap<PlayerIndex, Relationship>;

#[derive(Debug, Clone)]
pub struct PlayerRelationMap {
    pub map: PlayerRelationMapType,
}

impl PlayerRelationMap {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
}

/// Special power ready timer (matching C++ SpecialPowerReadyTimerType)
#[derive(Debug, Clone)]
pub struct SpecialPowerReadyTimer {
    pub(super) template_id: UnsignedInt,
    pub(super) ready_frame: UnsignedInt,
}

impl SpecialPowerReadyTimer {
    pub fn new() -> Self {
        Self {
            template_id: INVALID_ID,
            ready_frame: 0xffffffff,
        }
    }

    pub fn clear(&mut self) {
        self.ready_frame = 0xffffffff;
        self.template_id = INVALID_ID;
    }
}

/// Player template interface
#[derive(Debug, Clone)]
pub struct PlayerTemplate {
    /// C++ `m_nameKey` — copied from Common `PlayerTemplate::name_key`.
    pub name_key: NameKeyType,
    pub name: String,
    pub side: String,
    pub base_side: String,
    pub display_name: String,
    /// C++ `m_playableSide`. Default `false` matches `PlayerTemplate::PlayerTemplate()`.
    pub playable: bool,
    pub is_observer: bool,
    pub old_faction: bool,
    pub starting_money: Money,
    pub preferred_color: u32,
    pub starting_building: String,
    pub starting_units: Vec<String>,
    pub score_screen_image: String,
    pub score_screen_music: String,
    pub load_screen_image: String,
    pub load_screen_music: String,
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
    pub purchase_science_command_set_rank1: String,
    pub purchase_science_command_set_rank3: String,
    pub purchase_science_command_set_rank8: String,
    pub special_power_shortcut_command_set: String,
    pub special_power_shortcut_win_name: String,
    pub special_power_shortcut_button_count: Int,
    pub player_allies: String,
    pub player_enemies: String,
    pub(super) intrinsic_sciences: ScienceVec,
    pub(super) intrinsic_science_purchase_points: Int,
    pub(super) production_cost_changes: HashMap<NameKeyType, Real>,
    pub(super) production_time_changes: HashMap<NameKeyType, Real>,
    pub(super) production_veterancy_levels: HashMap<NameKeyType, VeterancyLevel>,
}

impl PlayerTemplate {
    pub fn new(name: String) -> Self {
        // C++ constructs with `NAMEKEY_INVALID` then `setNameKey` after `initFromINI`.
        // Bind the namekey here so `new(name)` matches Common's post-parse end-state.
        let name_key = if name.is_empty() {
            NAMEKEY_INVALID
        } else {
            NameKeyGenerator::name_to_key(&name)
        };
        Self {
            name_key,
            name,
            side: String::new(),
            base_side: String::new(),
            display_name: String::new(),
            playable: false,
            is_observer: false,
            old_faction: false,
            starting_money: Money::new(),
            preferred_color: 0,
            starting_building: String::new(),
            starting_units: vec![String::new(); MAX_MP_STARTING_UNITS],
            score_screen_image: String::new(),
            score_screen_music: String::new(),
            load_screen_image: String::new(),
            load_screen_music: String::new(),
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
            purchase_science_command_set_rank1: String::new(),
            purchase_science_command_set_rank3: String::new(),
            purchase_science_command_set_rank8: String::new(),
            special_power_shortcut_command_set: String::new(),
            special_power_shortcut_win_name: String::new(),
            special_power_shortcut_button_count: 0,
            player_allies: String::new(),
            player_enemies: String::new(),
            intrinsic_sciences: Vec::new(),
            intrinsic_science_purchase_points: 0,
            production_cost_changes: HashMap::new(),
            production_time_changes: HashMap::new(),
            production_veterancy_levels: HashMap::new(),
        }
    }

    pub fn from_common(
        template: &game_engine::common::rts::player_template::PlayerTemplate,
    ) -> Self {
        let mut result = Self::new(template.name.clone());
        result.apply_common(template);
        result
    }

    pub fn apply_common(
        &mut self,
        template: &game_engine::common::rts::player_template::PlayerTemplate,
    ) {
        self.name_key = template.name_key;
        self.name = template.name.clone();
        self.side = template.side.clone();
        self.base_side = template.base_side.clone();
        self.display_name = template.display_name.clone();
        self.playable = template.playable;
        self.is_observer = template.is_observer;
        self.old_faction = template.old_faction;
        self.starting_money = template.starting_money.clone();
        self.preferred_color = template.preferred_color;
        self.starting_building = template.starting_building.clone();
        // C++ `m_startingUnits[MAX_MP_STARTING_UNITS]` is a fixed array.
        self.starting_units = template.starting_units.clone();
        self.starting_units
            .resize(MAX_MP_STARTING_UNITS, String::new());
        self.score_screen_image = template.score_screen_image.clone();
        self.score_screen_music = template.score_screen_music.clone();
        self.load_screen_image = template.load_screen_image.clone();
        self.load_screen_music = template.load_screen_music.clone();
        self.head_water_mark = template.head_water_mark.clone();
        self.flag_water_mark = template.flag_water_mark.clone();
        self.enabled_image = template.enabled_image.clone();
        self.side_icon_image = template.side_icon_image.clone();
        self.general_image = template.general_image.clone();
        self.beacon_name = template.beacon_name.clone();
        self.army_tooltip = template.army_tooltip.clone();
        self.features = template.features.clone();
        self.medallion_regular = template.medallion_regular.clone();
        self.medallion_hilite = template.medallion_hilite.clone();
        self.medallion_select = template.medallion_select.clone();
        self.purchase_science_command_set_rank1 =
            template.purchase_science_command_set_rank1.clone();
        self.purchase_science_command_set_rank3 =
            template.purchase_science_command_set_rank3.clone();
        self.purchase_science_command_set_rank8 =
            template.purchase_science_command_set_rank8.clone();
        self.special_power_shortcut_command_set =
            template.special_power_shortcut_command_set.clone();
        self.special_power_shortcut_win_name = template.special_power_shortcut_win_name.clone();
        self.special_power_shortcut_button_count = template.special_power_shortcut_button_count;
        self.player_allies = template.player_allies.clone();
        self.player_enemies = template.player_enemies.clone();
        self.intrinsic_science_purchase_points = template.intrinsic_science_purchase_points;

        self.production_cost_changes = template.production_cost_changes.clone();
        self.production_time_changes = template.production_time_changes.clone();
        self.production_veterancy_levels = template
            .production_veterancy_levels
            .iter()
            .map(|(name_key, level)| {
                let mapped = match level {
                    game_engine::common::game_common::VeterancyLevel::Regular => {
                        crate::common::VeterancyLevel::Regular
                    }
                    game_engine::common::game_common::VeterancyLevel::Veteran => {
                        crate::common::VeterancyLevel::Veteran
                    }
                    game_engine::common::game_common::VeterancyLevel::Elite => {
                        crate::common::VeterancyLevel::Elite
                    }
                    game_engine::common::game_common::VeterancyLevel::Heroic => {
                        crate::common::VeterancyLevel::Heroic
                    }
                };
                (*name_key, mapped)
            })
            .collect();

        // C++ `parseScienceVector` → `friend_lookupScience` stores NAMEKEY as
        // ScienceType. Resolve via the store when present; otherwise use the
        // same namekey so from_common still copies INI sciences.
        self.intrinsic_sciences.clear();
        for name in template.get_intrinsic_sciences() {
            if name.is_empty() || name.eq_ignore_ascii_case("None") {
                continue;
            }
            let looked_up = get_science_store()
                .map(|store| store.get_science_from_internal_name(name))
                .unwrap_or(SCIENCE_INVALID);
            let science = if looked_up != SCIENCE_INVALID {
                looked_up
            } else {
                NameKeyGenerator::name_to_key(name) as ScienceType
            };
            self.intrinsic_sciences.push(science);
        }
    }

    pub fn hydrate_from_common_store(&mut self) {
        ensure_player_templates_loaded();
        let store = get_player_template_store();
        if let Some(found) = store.find_template(&self.name) {
            self.apply_common(found);
        } else if !self.name.is_empty() {
            log::warn!(
                "PlayerTemplate '{}' not found in store (map may be obsolete)",
                self.name
            );
        }
    }

    /// Get the side/faction name
    pub fn get_side(&self) -> &str {
        &self.side
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

    /// C++ `getDisplayName()`
    pub fn get_display_name(&self) -> &str {
        if self.display_name.is_empty() {
            &self.name
        } else {
            &self.display_name
        }
    }

    /// C++ `getBaseSide()`
    pub fn get_base_side(&self) -> &str {
        &self.base_side
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

    /// C++ `getPreferredColor()` — packed 0xRRGGBB.
    pub fn get_preferred_color(&self) -> u32 {
        self.preferred_color
    }

    /// C++ `isObserver()` → `m_observer`
    pub fn is_observer(&self) -> bool {
        self.is_observer
    }

    /// C++ `isOldFaction()`
    pub fn is_old_faction(&self) -> bool {
        self.old_faction
    }

    /// C++ `isPlayableSide()` → `m_playableSide` only.
    ///
    /// Zero Hour UI/ladder call sites additionally skip `side == "Boss"`. That
    /// filter is **not** part of this method; see [`is_playable_side_excluding_boss`].
    pub fn is_playable_side(&self) -> bool {
        self.playable
    }

    /// ZH shell/ladder filter: playable and not the hidden Boss side.
    ///
    /// C++ applies this at the call site, not on `PlayerTemplate::isPlayableSide`.
    pub fn is_playable_side_excluding_boss(&self) -> bool {
        self.is_playable_side() && self.side != "Boss"
    }

    /// C++ `getIntrinsicSciences()`
    pub fn get_intrinsic_sciences(&self) -> &ScienceVec {
        &self.intrinsic_sciences
    }

    /// C++ `getIntrinsicSciencePurchasePoints()`
    pub fn get_intrinsic_science_purchase_points(&self) -> Int {
        self.intrinsic_science_purchase_points
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
    pub fn get_special_power_shortcut_button_count(&self) -> Int {
        self.special_power_shortcut_button_count
    }

    pub fn get_score_screen(&self) -> &str {
        &self.score_screen_image
    }

    pub fn get_score_screen_music(&self) -> &str {
        &self.score_screen_music
    }

    /// C++ `getLoadScreen()`
    pub fn get_load_screen(&self) -> &str {
        &self.load_screen_image
    }

    /// C++ `getLoadScreenMusic()`
    pub fn get_load_screen_music(&self) -> &str {
        &self.load_screen_music
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

    pub fn get_side_icon_image(&self) -> &str {
        &self.side_icon_image
    }

    pub fn get_head_water_mark(&self) -> &str {
        &self.head_water_mark
    }

    pub fn get_flag_water_mark(&self) -> &str {
        &self.flag_water_mark
    }

    pub fn get_enabled_image(&self) -> &str {
        &self.enabled_image
    }

    pub fn get_general_image(&self) -> &str {
        &self.general_image
    }

    /// C++ `getProductionCostChanges()`
    pub fn get_production_cost_changes(&self) -> &HashMap<NameKeyType, Real> {
        &self.production_cost_changes
    }

    /// C++ `getProductionTimeChanges()`
    pub fn get_production_time_changes(&self) -> &HashMap<NameKeyType, Real> {
        &self.production_time_changes
    }

    /// C++ `getProductionVeterancyLevels()`
    pub fn get_production_veterancy_levels(&self) -> &HashMap<NameKeyType, VeterancyLevel> {
        &self.production_veterancy_levels
    }

    pub fn production_cost_changes(&self) -> &HashMap<NameKeyType, Real> {
        self.get_production_cost_changes()
    }

    pub fn production_time_changes(&self) -> &HashMap<NameKeyType, Real> {
        self.get_production_time_changes()
    }

    pub fn production_veterancy_levels(&self) -> &HashMap<NameKeyType, VeterancyLevel> {
        self.get_production_veterancy_levels()
    }
}

#[derive(Debug, Clone)]
pub(super) struct KindOfPercentProductionChange {
    pub(super) kind_of: KindOfMaskType,
    pub(super) percent: Real,
    pub(super) refs: u32,
}

