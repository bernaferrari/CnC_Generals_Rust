//! C++ `StealthUpdate` / `StealthUpdateModuleData`
//! (GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/StealthUpdate.cpp,
//! GeneralsMD/Code/GameEngine/Include/GameLogic/Module/StealthUpdate.h).
//!
//! The module data is parsed once into the `ThingTemplate` from the authored
//! `Behavior = StealthUpdate` block. Every per-object stealth decision reads
//! that data; no template name selects stealth behavior. The per-object
//! runtime state is the C++ `m_stealthAllowedFrame` / `m_detectionExpiresFrame`
//! / `m_nextBlackMarketCheckFrame` / `m_pulsePhase` members and the
//! `OBJECT_STATUS_CAN_STEALTH` bit (`Object::innate_stealth`).

use super::*;
use crate::assets::ini_parser::BehaviorModuleDefinition;
use game_engine::common::ini::INI;

/// C++ `StealthLevel` bits in `TheStealthLevelNames` order (StealthUpdate.h:43-62).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StealthForbiddenConditions(u32);

impl StealthForbiddenConditions {
    pub const ATTACKING: Self = Self(1 << 0);
    pub const MOVING: Self = Self(1 << 1);
    pub const USING_ABILITY: Self = Self(1 << 2);
    pub const FIRING_PRIMARY: Self = Self(1 << 3);
    pub const FIRING_SECONDARY: Self = Self(1 << 4);
    pub const FIRING_TERTIARY: Self = Self(1 << 5);
    pub const NO_BLACK_MARKET: Self = Self(1 << 6);
    pub const TAKING_DAMAGE: Self = Self(1 << 7);
    pub const RIDERS_ATTACKING: Self = Self(1 << 8);
    /// C++ `STEALTH_NOT_WHILE_FIRING_WEAPON`.
    pub const FIRING_WEAPON: Self = Self(
        Self::FIRING_PRIMARY.0 | Self::FIRING_SECONDARY.0 | Self::FIRING_TERTIARY.0,
    );

    const NAMES: [(&'static str, Self); 9] = [
        ("ATTACKING", Self::ATTACKING),
        ("MOVING", Self::MOVING),
        ("USING_ABILITY", Self::USING_ABILITY),
        ("FIRING_PRIMARY", Self::FIRING_PRIMARY),
        ("FIRING_SECONDARY", Self::FIRING_SECONDARY),
        ("FIRING_TERTIARY", Self::FIRING_TERTIARY),
        ("NO_BLACK_MARKET", Self::NO_BLACK_MARKET),
        ("TAKING_DAMAGE", Self::TAKING_DAMAGE),
        ("RIDERS_ATTACKING", Self::RIDERS_ATTACKING),
    ];

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Any bit of `other` is set.
    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    /// Every bit of `other` is set.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// C++ `INI::parseBitString32` against `TheStealthLevelNames`
    /// (INI.cpp:880-910): `NONE` clears, names OR together. Unknown names
    /// reject the value.
    pub fn parse(value: &str) -> Option<Self> {
        let mut bits = Self::empty();
        for token in value.split_whitespace() {
            if token.eq_ignore_ascii_case("NONE") {
                bits = Self::empty();
                continue;
            }
            let (_, flag) = Self::NAMES
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(token))?;
            bits = bits.union(*flag);
        }
        Some(bits)
    }
}

/// C++ `HintDetectableConditions` (an `ObjectStatusMaskType`) over the two
/// statuses the host tracks for stealth hints.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StealthHintConditions {
    /// `IS_FIRING_WEAPON`.
    pub is_firing_weapon: bool,
    /// `USING_ABILITY`.
    pub is_using_ability: bool,
}

impl StealthHintConditions {
    /// C++ `ObjectStatusMaskType::parseFromINI`; status names outside the
    /// two tracked bits contribute nothing.
    fn parse(value: &str) -> Self {
        let mut hint = Self::default();
        for token in value.split_whitespace() {
            if token.eq_ignore_ascii_case("IS_FIRING_WEAPON") {
                hint.is_firing_weapon = true;
            } else if token.eq_ignore_ascii_case("USING_ABILITY") {
                hint.is_using_ability = true;
            }
        }
        hint
    }
}

/// C++ `StealthUpdateModuleData` (StealthUpdate.cpp:45-104).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StealthUpdateData {
    /// `StealthDelay` (parseDurationUnsignedInt). C++ default `UINT_MAX`.
    pub stealth_delay_frames: u32,
    /// `StealthForbiddenConditions` (`m_stealthLevel`).
    pub forbidden_conditions: StealthForbiddenConditions,
    /// `MoveThresholdSpeed` (parseVelocityReal, distance per frame).
    pub move_threshold_speed: f32,
    /// `HintDetectableConditions`.
    pub hint_detectable: StealthHintConditions,
    /// Status bits all required for stealth.
    pub required_status: u64,
    /// Status bits that prohibit stealth when any is set.
    pub forbidden_status: u64,
    /// `FriendlyOpacityMin` (parsePercentToReal).
    pub friendly_opacity_min: f32,
    /// `FriendlyOpacityMax` (parsePercentToReal).
    pub friendly_opacity_max: f32,
    /// `PulseFrequency` (parseDurationUnsignedInt).
    pub pulse_frames: u32,
    /// Authored gain and reveal transition durations in logic frames.
    pub disguise_transition_frames: u32,
    pub disguise_reveal_transition_frames: u32,
    /// `DisguisesAsTeam` (`m_teamDisguised`).
    pub disguises_as_team: bool,
    /// `RevealDistanceFromTarget`.
    pub reveal_distance_from_target: f32,
    /// `OrderIdleEnemiesToAttackMeUponReveal`.
    pub order_idle_enemies_to_attack_me_upon_reveal: bool,
    /// `InnateStealth`. C++ default true.
    pub innate_stealth: bool,
    /// `UseRiderStealth`.
    pub use_rider_stealth: bool,
    /// `BlackMarketCheckDelay` (parseDurationUnsignedInt).
    pub black_market_check_frames: u32,
    /// `GrantedBySpecialPower`: the module sleeps until `receiveGrant`.
    pub granted_by_special_power: bool,
}

impl Default for StealthUpdateData {
    /// C++ `StealthUpdateModuleData::StealthUpdateModuleData` (StealthUpdate.cpp:45-68).
    fn default() -> Self {
        Self {
            stealth_delay_frames: u32::MAX,
            forbidden_conditions: StealthForbiddenConditions::empty(),
            move_threshold_speed: 0.0,
            hint_detectable: StealthHintConditions::default(),
            required_status: 0,
            forbidden_status: 0,
            friendly_opacity_min: 0.5,
            friendly_opacity_max: 1.0,
            pulse_frames: 30,
            disguise_transition_frames: 0,
            disguise_reveal_transition_frames: 0,
            disguises_as_team: false,
            reveal_distance_from_target: 0.0,
            order_idle_enemies_to_attack_me_upon_reveal: false,
            innate_stealth: true,
            use_rider_stealth: false,
            black_market_check_frames: 0,
            granted_by_special_power: false,
        }
    }
}

impl StealthUpdateData {
    /// Copy the effective typed module selected by ThingTemplate inheritance.
    pub(crate) fn from_admitted_module(module: &gamelogic::stealth_update::StealthUpdateModuleData) -> Self {
        use gamelogic::common::types::ObjectStatusMaskType;
        let data = module.authored_fields();
        Self {
            stealth_delay_frames: data.stealth_delay_frames,
            forbidden_conditions: StealthForbiddenConditions(data.stealth_level_mask),
            move_threshold_speed: data.stealth_speed,
            hint_detectable: StealthHintConditions {
                is_firing_weapon: data.hint_detectable_states.contains(ObjectStatusMaskType::IS_FIRING_WEAPON),
                is_using_ability: data.hint_detectable_states.contains(ObjectStatusMaskType::IS_USING_ABILITY),
            },
            required_status: data.required_status.bits(),
            forbidden_status: data.forbidden_status.bits(),
            friendly_opacity_min: data.friendly_opacity_min,
            friendly_opacity_max: data.friendly_opacity_max,
            pulse_frames: data.pulse_frames,
            disguise_transition_frames: data.disguise_transition_frames,
            disguise_reveal_transition_frames: data.disguise_reveal_transition_frames,
            disguises_as_team: data.team_disguised,
            reveal_distance_from_target: data.reveal_distance_from_target,
            order_idle_enemies_to_attack_me_upon_reveal: data.order_idle_enemies_to_attack,
            innate_stealth: data.innate_stealth,
            use_rider_stealth: data.use_rider_stealth,
            black_market_check_frames: data.black_market_check_frames,
            granted_by_special_power: data.granted_by_special_power,
        }
    }

    /// Parse one authored `Behavior = StealthUpdate` block with the C++
    /// field parsers (StealthUpdate.cpp:72-104). A malformed value keeps the
    /// constructor default, as an INI parse failure leaves the field untouched.
    pub fn from_module(module: &BehaviorModuleDefinition) -> Self {
        let mut data = Self::default();
        let field = |name: &str| {
            module
                .attribute(name)
                .map(|value| value.split(';').next().unwrap_or_default().trim())
                .filter(|value| !value.is_empty())
        };
        let first = |name: &str| field(name).and_then(|value| value.split_whitespace().next());
        if let Some(v) = first("StealthDelay").and_then(|v| INI::parse_duration_unsigned_int(v).ok()) {
            data.stealth_delay_frames = v;
        }
        if let Some(v) = first("MoveThresholdSpeed").and_then(|v| INI::parse_velocity_real(v).ok()) {
            data.move_threshold_speed = v;
        }
        if let Some(v) = field("StealthForbiddenConditions").and_then(StealthForbiddenConditions::parse) {
            data.forbidden_conditions = v;
        }
        if let Some(v) = field("HintDetectableConditions") {
            data.hint_detectable = StealthHintConditions::parse(v);
        }
        if let Some(v) = field("RequiredStatus")
            .and_then(crate::game_logic::host_temporary_weapon_behavior::parse_cxx_object_status_mask)
        {
            data.required_status = v.0;
        }
        if let Some(v) = field("ForbiddenStatus")
            .and_then(crate::game_logic::host_temporary_weapon_behavior::parse_cxx_object_status_mask)
        {
            data.forbidden_status = v.0;
        }
        if let Some(v) = first("FriendlyOpacityMin").and_then(|v| INI::parse_percent_to_real(v).ok()) {
            data.friendly_opacity_min = v;
        }
        if let Some(v) = first("FriendlyOpacityMax").and_then(|v| INI::parse_percent_to_real(v).ok()) {
            data.friendly_opacity_max = v;
        }
        if let Some(v) = first("PulseFrequency").and_then(|v| INI::parse_duration_unsigned_int(v).ok()) {
            data.pulse_frames = v;
        }
        if let Some(v) = first("DisguiseTransitionTime").and_then(|v| INI::parse_duration_unsigned_int(v).ok()) {
            data.disguise_transition_frames = v;
        }
        if let Some(v) = first("DisguiseRevealTransitionTime").and_then(|v| INI::parse_duration_unsigned_int(v).ok()) {
            data.disguise_reveal_transition_frames = v;
        }
        let flag = |name: &str| first(name).and_then(|v| INI::parse_bool(v).ok());
        if let Some(v) = flag("DisguisesAsTeam") {
            data.disguises_as_team = v;
        }
        if let Some(v) = first("RevealDistanceFromTarget").and_then(|v| INI::parse_real(v).ok()) {
            data.reveal_distance_from_target = v;
        }
        if let Some(v) = flag("OrderIdleEnemiesToAttackMeUponReveal") {
            data.order_idle_enemies_to_attack_me_upon_reveal = v;
        }
        if let Some(v) = flag("InnateStealth") {
            data.innate_stealth = v;
        }
        if let Some(v) = flag("UseRiderStealth") {
            data.use_rider_stealth = v;
        }
        if let Some(v) = first("BlackMarketCheckDelay").and_then(|v| INI::parse_duration_unsigned_int(v).ok()) {
            data.black_market_check_frames = v;
        }
        if let Some(v) = flag("GrantedBySpecialPower") {
            data.granted_by_special_power = v;
        }
        data
    }
}

/// Inputs to C++ `StealthUpdate::allowedToStealth` that live outside the
/// stealthing Object (StealthUpdate.cpp:234-401).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct StealthWorldFacts {
    /// `m_stealthLevel` of the stealth owner (the first rider under
    /// `UseRiderStealth`, else this Object).
    pub flags: StealthForbiddenConditions,
    /// The stealth owner holds `OBJECT_STATUS_CAN_STEALTH`.
    pub owner_can_stealth: bool,
    /// The due `NO_BLACK_MARKET` check (`m_nextBlackMarketCheckFrame < now`)
    /// found no live `KINDOF_FS_BLACK_MARKET` of the controlling player
    /// (`isBlackMarket`). A check that is not due never blocks.
    pub black_market_missing: bool,
    /// Contained by a container whose contain is not garrisonable.
    pub contained_by_non_garrisonable: bool,
    /// `isPassengerAllowedToFire() && isAnyRiderAttacking()` on this Object's contain.
    pub riders_attacking: bool,
}

impl Object {
    /// C++ `Object::getStealth()` reads the actual admitted module.
    pub fn stealth_update_data(&self) -> Option<&StealthUpdateData> {
        self.thing.template.stealth_update.as_ref()
    }

    /// C++ `StealthUpdate::canDisguise()` (`m_teamDisguised`, the authored
    /// `DisguisesAsTeam`).
    pub fn stealth_can_disguise(&self) -> bool {
        self.stealth_update_data()
            .is_some_and(|data| data.disguises_as_team)
    }

    /// C++ `StealthUpdate` constructor (StealthUpdate.cpp:107-147):
    /// `m_stealthAllowedFrame = now + StealthDelay` (unsigned wrap, as C++)
    /// and `OBJECT_STATUS_CAN_STEALTH` for `InnateStealth`.
    pub(crate) fn construct_stealth_update(&mut self, now: u32) {
        let Some(data) = self.stealth_update_data().copied() else {
            return;
        };
        self.stealth_allowed_frame = now.wrapping_add(data.stealth_delay_frames);
        self.stealth_update_enabled = !data.disguises_as_team;
        self.stealth_update_awake = !data.granted_by_special_power;
        if data.innate_stealth {
            self.innate_stealth = true;
        }
    }

    /// C++ `StealthUpdate::getFriendlyOpacity` (`FriendlyOpacityMin`), the
    /// module-data default without a module.
    pub fn stealth_friendly_opacity_min(&self) -> f32 {
        self.stealth_update_data()
            .map_or(StealthUpdateData::default().friendly_opacity_min, |d| d.friendly_opacity_min)
    }

    /// StealthUpdate `FriendlyOpacityMax`, the module-data default without a module.
    pub fn stealth_friendly_opacity_max(&self) -> f32 {
        self.stealth_update_data()
            .map_or(StealthUpdateData::default().friendly_opacity_max, |d| d.friendly_opacity_max)
    }

    /// Whether the enabled module is awake in the update scheduler.
    pub(crate) fn stealth_update_runs(&self, _data: &StealthUpdateData) -> bool {
        self.stealth_update_enabled && self.stealth_update_awake
    }

    /// C++ `Weapon::getLastShotFrame() >= now - 1` for `slot`.
    fn fired_slot_since_last_frame(&self, slot: u8, now: u32) -> bool {
        self.weapon_slot(slot)
            .is_some_and(|weapon| weapon.last_fire_frame >= now.wrapping_sub(1))
    }

    /// C++ `StealthUpdate::allowedToStealth` (StealthUpdate.cpp:234-401) for
    /// this Object; `facts` carries the stealth owner and contain inputs.
    pub(crate) fn allowed_to_stealth(
        &self,
        data: &StealthUpdateData,
        facts: StealthWorldFacts,
        now: u32,
    ) -> bool {
        type C = StealthForbiddenConditions;
        let flags = facts.flags;
        let firing = self.status.is_firing_weapon;
        if flags.intersects(C::ATTACKING) && firing {
            return false;
        }
        if flags.intersects(C::USING_ABILITY) && self.status.using_ability {
            return false;
        }
        if flags.intersects(C::NO_BLACK_MARKET) && facts.black_market_missing {
            return false;
        }
        if !facts.owner_can_stealth {
            return false;
        }
        if flags.intersects(C::TAKING_DAMAGE) && self.stealth_taking_non_healing_damage(now) {
            return false;
        }
        if data.required_status != 0 || data.forbidden_status != 0 {
            let statuses = self.structure_topple_status_mask().bits();
            if statuses & data.required_status != data.required_status
                || statuses & data.forbidden_status != 0
            {
                return false;
            }
        }
        if flags.intersects(C::FIRING_WEAPON) && firing {
            if flags.contains(C::FIRING_WEAPON) {
                return false;
            }
            for (flag, slot) in [(C::FIRING_PRIMARY, 0), (C::FIRING_SECONDARY, 1), (C::FIRING_TERTIARY, 2)] {
                if flags.intersects(flag) && self.fired_slot_since_last_frame(slot, now) {
                    return false;
                }
            }
        }
        if facts.contained_by_non_garrisonable {
            return false;
        }
        if flags.intersects(C::RIDERS_ATTACKING) && facts.riders_attacking {
            return false;
        }
        // PhysicsBehavior::getVelocityMagnitude.
        if flags.intersects(C::MOVING) && self.movement.velocity.length() > data.move_threshold_speed
        {
            return false;
        }
        !self.script_unstealthed
    }

    /// C++ `StealthUpdate::update` cloak branch (StealthUpdate.cpp:717-752):
    /// cloak once `m_stealthAllowedFrame` has passed while allowed; otherwise
    /// roll `m_stealthAllowedFrame = now + stealthDelay` and drop STEALTHED.
    pub(crate) fn apply_stealth_allowed(&mut self, now: u32, allowed: bool, stealth_delay: u32) {
        if allowed {
            if self.stealth_allowed_frame > now {
                return;
            }
            self.set_status_stealthed(true);
        } else {
            self.stealth_allowed_frame = now.wrapping_add(stealth_delay);
            self.set_status_stealthed(false);
        }
    }

    /// C++ `StealthUpdate::markAsDetected` (StealthUpdate.cpp:846-891)
    /// detection timer: `numFrames == 0` assigns `now + stealthDelay`,
    /// otherwise the later of the current expiry and `now + numFrames`.
    /// Removes a disguise first.
    pub(crate) fn mark_as_detected_for(&mut self, now: u32, num_frames: u32, stealth_delay: u32) {
        if self.is_disguised() {
            self.clear_disguise();
        }
        if num_frames == 0 {
            self.detection_expires_frame = now.wrapping_add(stealth_delay);
        } else if self.detection_expires_frame < now.wrapping_add(num_frames) {
            self.detection_expires_frame = now.wrapping_add(num_frames);
        }
    }
}
