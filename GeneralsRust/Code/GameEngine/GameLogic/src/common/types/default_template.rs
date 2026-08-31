// DefaultThingTemplate and Arc wrappers
//
// Split from `types.rs` for module-size parity.
// Observable behavior is unchanged.

/// Default thing template implementation
#[derive(Debug, Clone)]
pub struct DefaultThingTemplate {
    name: AsciiString,
    geometry_info: GeometryInfo,
    vision_range: Real,
    shroud_clearing_range: Real,
    kind_of_flags: Vec<KindOf>,
    build_cost: Int,
    build_time: Real,
    threat_value: UnsignedInt,
    crusher_level: u32,
    crushable_level: u32,
    transport_slot_count: UnsignedByte,
    shroud_reveal_to_all_range: Real,
    occlusion_delay: u32,
    max_health: Real,
    energy_production: Int,
    energy_bonus: Int,
    command_set_string: AsciiString,
    armor_template_sets: Vec<ArmorTemplateSet>,

    // --- Audio: per-unit sounds / FX (key = condition name) ---
    per_unit_sounds: HashMap<String, crate::common::audio::AudioEventRts>,
    per_unit_fx: HashMap<String, crate::common::audio::AudioEventRts>,

    // --- Audio: voice events (TTAUDIO_voice*) ---
    voice_select: crate::common::audio::AudioEventRts,
    voice_group_select: crate::common::audio::AudioEventRts,
    voice_move: crate::common::audio::AudioEventRts,
    voice_attack: crate::common::audio::AudioEventRts,
    voice_enter: crate::common::audio::AudioEventRts,
    voice_fear: crate::common::audio::AudioEventRts,
    voice_select_elite: crate::common::audio::AudioEventRts,
    voice_created: crate::common::audio::AudioEventRts,
    voice_task_unable: crate::common::audio::AudioEventRts,
    voice_task_complete: crate::common::audio::AudioEventRts,
    voice_meet_enemy: crate::common::audio::AudioEventRts,
    voice_garrison: crate::common::audio::AudioEventRts,
    voice_defect: crate::common::audio::AudioEventRts,
    voice_attack_special: crate::common::audio::AudioEventRts,
    voice_attack_air: crate::common::audio::AudioEventRts,
    voice_guard: crate::common::audio::AudioEventRts,

    // --- Audio: sound events (TTAUDIO_sound*) ---
    sound_move_start: crate::common::audio::AudioEventRts,
    sound_move_start_damaged: crate::common::audio::AudioEventRts,
    sound_move_loop: crate::common::audio::AudioEventRts,
    sound_move_loop_damaged: crate::common::audio::AudioEventRts,
    sound_ambient: crate::common::audio::AudioEventRts,
    sound_ambient_damaged: crate::common::audio::AudioEventRts,
    sound_ambient_really_damaged: crate::common::audio::AudioEventRts,
    sound_ambient_rubble: crate::common::audio::AudioEventRts,
    sound_stealth_on: crate::common::audio::AudioEventRts,
    sound_stealth_off: crate::common::audio::AudioEventRts,
    sound_created: crate::common::audio::AudioEventRts,
    sound_on_damaged: crate::common::audio::AudioEventRts,
    sound_on_really_damaged: crate::common::audio::AudioEventRts,
    sound_enter: crate::common::audio::AudioEventRts,
    sound_exit: crate::common::audio::AudioEventRts,
    sound_promoted_veteran: crate::common::audio::AudioEventRts,
    sound_promoted_elite: crate::common::audio::AudioEventRts,
    sound_promoted_hero: crate::common::audio::AudioEventRts,
    sound_falling: crate::common::audio::AudioEventRts,
}

impl DefaultThingTemplate {
    pub fn new(name: String) -> Self {
        let audio_default = crate::common::audio::AudioEventRts::default;
        Self {
            name: AsciiString::from(&name),
            geometry_info: GeometryInfo::default(),
            vision_range: 100.0,
            shroud_clearing_range: -1.0,
            kind_of_flags: Vec::new(),
            build_cost: 0,
            build_time: 0.0,
            threat_value: 0,
            crusher_level: 0,
            crushable_level: 255,
            transport_slot_count: 0,
            shroud_reveal_to_all_range: 0.0,
            occlusion_delay: global_data::read().default_occlusion_delay,
            max_health: 100.0,
            energy_production: 0,
            energy_bonus: 0,
            command_set_string: AsciiString::new(),
            armor_template_sets: Vec::new(),
            per_unit_sounds: HashMap::new(),
            per_unit_fx: HashMap::new(),
            // Voice events
            voice_select: audio_default(),
            voice_group_select: audio_default(),
            voice_move: audio_default(),
            voice_attack: audio_default(),
            voice_enter: audio_default(),
            voice_fear: audio_default(),
            voice_select_elite: audio_default(),
            voice_created: audio_default(),
            voice_task_unable: audio_default(),
            voice_task_complete: audio_default(),
            voice_meet_enemy: audio_default(),
            voice_garrison: audio_default(),
            voice_defect: audio_default(),
            voice_attack_special: audio_default(),
            voice_attack_air: audio_default(),
            voice_guard: audio_default(),
            // Sound events
            sound_move_start: audio_default(),
            sound_move_start_damaged: audio_default(),
            sound_move_loop: audio_default(),
            sound_move_loop_damaged: audio_default(),
            sound_ambient: audio_default(),
            sound_ambient_damaged: audio_default(),
            sound_ambient_really_damaged: audio_default(),
            sound_ambient_rubble: audio_default(),
            sound_stealth_on: audio_default(),
            sound_stealth_off: audio_default(),
            sound_created: audio_default(),
            sound_on_damaged: audio_default(),
            sound_on_really_damaged: audio_default(),
            sound_enter: audio_default(),
            sound_exit: audio_default(),
            sound_promoted_veteran: audio_default(),
            sound_promoted_elite: audio_default(),
            sound_promoted_hero: audio_default(),
            sound_falling: audio_default(),
        }
    }

    pub fn set_max_health(&mut self, max_health: Real) {
        self.max_health = max_health.max(0.0);
    }

    pub fn set_build_time(&mut self, build_time: Real) {
        self.build_time = build_time.max(0.0);
    }

    pub fn set_threat_value(&mut self, threat_value: UnsignedInt) {
        self.threat_value = threat_value;
    }

    pub fn set_crusher_level(&mut self, crusher_level: u32) {
        self.crusher_level = crusher_level.min(u8::MAX as u32);
    }

    pub fn set_crushable_level(&mut self, crushable_level: u32) {
        self.crushable_level = crushable_level.min(u8::MAX as u32);
    }

    pub fn set_shroud_reveal_to_all_range(&mut self, range: Real) {
        self.shroud_reveal_to_all_range = range.max(0.0);
    }

    pub fn set_occlusion_delay(&mut self, delay: u32) {
        self.occlusion_delay = delay;
    }

    pub fn set_energy_production(&mut self, energy: Int) {
        self.energy_production = energy;
    }

    pub fn set_energy_bonus(&mut self, bonus: Int) {
        self.energy_bonus = bonus;
    }

    pub fn add_armor_template_set(&mut self, set: ArmorTemplateSet) {
        self.armor_template_sets.push(set);
    }

    /// Add a KINDOF flag used by OCL LIKE_EXISTING structure flattening tests.
    pub fn add_kind_of(&mut self, kind: KindOf) {
        if !self.kind_of_flags.contains(&kind) {
            self.kind_of_flags.push(kind);
        }
    }

    pub fn set_per_unit_sound(
        &mut self,
        name: impl Into<String>,
        sound: crate::common::audio::AudioEventRts,
    ) {
        self.per_unit_sounds.insert(name.into(), sound);
    }

    pub fn set_per_unit_fx(
        &mut self,
        name: impl Into<String>,
        fx: crate::common::audio::AudioEventRts,
    ) {
        self.per_unit_fx.insert(name.into(), fx);
    }

    pub fn set_voice_attack(&mut self, sound: crate::common::audio::AudioEventRts) {
        self.voice_attack = sound;
    }

    pub fn set_voice_attack_special(&mut self, sound: crate::common::audio::AudioEventRts) {
        self.voice_attack_special = sound;
    }

    pub fn set_voice_attack_air(&mut self, sound: crate::common::audio::AudioEventRts) {
        self.voice_attack_air = sound;
    }

    /// Helper: set a voice field from an INI string value.
    fn set_voice_from_ini(&mut self, field: &str, value: &str) {
        let audio = crate::common::audio::AudioEventRts::new(value);
        match field {
            "VoiceSelect" => self.voice_select = audio,
            "VoiceGroupSelect" => self.voice_group_select = audio,
            "VoiceMove" => self.voice_move = audio,
            "VoiceAttack" => self.voice_attack = audio,
            "VoiceEnter" => self.voice_enter = audio,
            "VoiceFear" => self.voice_fear = audio,
            "VoiceSelectElite" => self.voice_select_elite = audio,
            "VoiceCreated" => self.voice_created = audio,
            "VoiceTaskUnable" => self.voice_task_unable = audio,
            "VoiceTaskComplete" => self.voice_task_complete = audio,
            "VoiceMeetEnemy" => self.voice_meet_enemy = audio,
            "VoiceGarrison" => self.voice_garrison = audio,
            "VoiceDefect" => self.voice_defect = audio,
            "VoiceAttackSpecial" => self.voice_attack_special = audio,
            "VoiceAttackAir" => self.voice_attack_air = audio,
            "VoiceGuard" => self.voice_guard = audio,
            _ => {}
        }
    }

    /// Helper: set a sound field from an INI string value.
    fn set_sound_from_ini(&mut self, field: &str, value: &str) {
        let audio = crate::common::audio::AudioEventRts::new(value);
        match field {
            "SoundMoveStart" => self.sound_move_start = audio,
            "SoundMoveStartDamaged" => self.sound_move_start_damaged = audio,
            "SoundMoveLoop" => self.sound_move_loop = audio,
            "SoundMoveLoopDamaged" => self.sound_move_loop_damaged = audio,
            "SoundAmbient" => self.sound_ambient = audio,
            "SoundAmbientDamaged" => self.sound_ambient_damaged = audio,
            "SoundAmbientReallyDamaged" => self.sound_ambient_really_damaged = audio,
            "SoundAmbientRubble" => self.sound_ambient_rubble = audio,
            "SoundStealthOn" => self.sound_stealth_on = audio,
            "SoundStealthOff" => self.sound_stealth_off = audio,
            "SoundCreated" => self.sound_created = audio,
            "SoundOnDamaged" => self.sound_on_damaged = audio,
            "SoundOnReallyDamaged" => self.sound_on_really_damaged = audio,
            "SoundEnter" => self.sound_enter = audio,
            "SoundExit" => self.sound_exit = audio,
            "SoundPromotedVeteran" => self.sound_promoted_veteran = audio,
            "SoundPromotedElite" => self.sound_promoted_elite = audio,
            "SoundPromotedHero" => self.sound_promoted_hero = audio,
            "SoundFallingFromPlane" => self.sound_falling = audio,
            _ => {}
        }
    }

    pub fn set_command_set_string(&mut self, command_set: AsciiString) {
        self.command_set_string = command_set;
    }

    fn kind_of_mask(&self) -> KindOfMaskType {
        let mut mask: KindOfMaskType = KIND_OF_MASK_NONE;
        for &kind in ALL_KIND_OF {
            if self.is_kind_of(kind) {
                mask |= kind.cpp_mask();
            }
        }
        mask
    }

    pub fn find_armor_template_set(&self, flags: &ArmorSetBitFlags) -> Option<&ArmorTemplateSet> {
        // C++ SparseMatchFinder.h:99-143 — most matching yes-bits, then fewest extras.
        let mut best: Option<&ArmorTemplateSet> = None;
        let mut best_yes_match = 0usize;
        let mut best_yes_extraneous = usize::MAX;
        for set in &self.armor_template_sets {
            let yes_flags = set.types();
            let yes_match = flags.count_intersection(yes_flags);
            let yes_extraneous = flags.count_inverse_intersection(yes_flags);
            if yes_match > best_yes_match
                || (yes_match >= best_yes_match && yes_extraneous < best_yes_extraneous)
            {
                best = Some(set);
                best_yes_match = yes_match;
                best_yes_extraneous = yes_extraneous;
            }
        }
        best
    }

    /// Apply parsed INI key=value properties to this template.
    ///
    /// This is the GameLogic-side equivalent of `parse_object_fields_from_ini`
    /// in the Common crate ThingTemplate.  It handles fields that are specific
    /// to the GameLogic layer (KindOf, MaxHealth, Armor, etc.) and delegates
    /// the rest to the Common-layer parsing.
    ///
    /// C++ Reference: ThingTemplate::s_objectFieldParseTable (ThingTemplate.cpp:90-229)
    pub fn parse_object_fields_from_ini(
        &mut self,
        properties: &std::collections::HashMap<String, String>,
    ) {
        for (key, value) in properties {
            let trimmed = value.trim();
            match key.as_str() {
                // --- Vision / shroud ---
                "VisionRange" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.vision_range = v;
                    }
                }
                "ShroudClearingRange" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.shroud_clearing_range = v;
                    }
                }
                "ShroudRevealToAllRange" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.shroud_reveal_to_all_range = v;
                    }
                }

                // --- Build ---
                "BuildCost" => {
                    if let Ok(v) = trimmed.parse::<Int>() {
                        self.build_cost = v;
                    }
                }
                "BuildTime" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.build_time = v;
                    }
                }

                // --- Combat ---
                "ThreatValue" => {
                    if let Ok(v) = trimmed.parse::<UnsignedInt>() {
                        self.threat_value = v;
                    }
                }
                "TransportSlotCount" => {
                    if let Ok(v) = trimmed.parse::<UnsignedByte>() {
                        self.transport_slot_count = v;
                    }
                }

                // --- Occlusion ---
                "OcclusionDelay" => {
                    if let Ok(v) = trimmed.parse::<u32>() {
                        self.occlusion_delay = v;
                    }
                }

                // --- Energy ---
                "EnergyProduction" => {
                    if let Ok(v) = trimmed.parse::<Int>() {
                        self.energy_production = v;
                    }
                }
                "EnergyBonus" => {
                    if let Ok(v) = trimmed.parse::<Int>() {
                        self.energy_bonus = v;
                    }
                }

                // --- Command set ---
                "CommandSet" => {
                    self.command_set_string = AsciiString::from(trimmed);
                }

                // --- KindOf ---
                "KindOf" => {
                    // Parse KindOf flags. C++ INI accepts whitespace-separated lists;
                    // existing Rust data also allowed pipe separators.
                    // C++ uses KindOfMaskType::parseFromINI
                    self.kind_of_flags.clear();
                    for token in trimmed.split(|c: char| c == '|' || c.is_ascii_whitespace()) {
                        let t = token.trim();
                        if t.is_empty() {
                            continue;
                        }
                        if let Some(kind) = kindof_from_name(t) {
                            self.kind_of_flags.push(kind);
                        }
                    }
                }

                // --- Voice events (C++ TTAUDIO_voice*) ---
                // Parsed via INI::parseDynamicAudioEventRTS in C++
                key @ "VoiceSelect"
                | key @ "VoiceGroupSelect"
                | key @ "VoiceMove"
                | key @ "VoiceAttack"
                | key @ "VoiceEnter"
                | key @ "VoiceFear"
                | key @ "VoiceSelectElite"
                | key @ "VoiceCreated"
                | key @ "VoiceTaskUnable"
                | key @ "VoiceTaskComplete"
                | key @ "VoiceMeetEnemy"
                | key @ "VoiceGarrison"
                | key @ "VoiceDefect"
                | key @ "VoiceAttackSpecial"
                | key @ "VoiceAttackAir"
                | key @ "VoiceGuard" => {
                    self.set_voice_from_ini(key, trimmed);
                }

                // --- Sound events (C++ TTAUDIO_sound*) ---
                // Parsed via INI::parseDynamicAudioEventRTS in C++
                key @ "SoundMoveStart"
                | key @ "SoundMoveStartDamaged"
                | key @ "SoundMoveLoop"
                | key @ "SoundMoveLoopDamaged"
                | key @ "SoundAmbient"
                | key @ "SoundAmbientDamaged"
                | key @ "SoundAmbientReallyDamaged"
                | key @ "SoundAmbientRubble"
                | key @ "SoundStealthOn"
                | key @ "SoundStealthOff"
                | key @ "SoundCreated"
                | key @ "SoundOnDamaged"
                | key @ "SoundOnReallyDamaged"
                | key @ "SoundEnter"
                | key @ "SoundExit"
                | key @ "SoundPromotedVeteran"
                | key @ "SoundPromotedElite"
                | key @ "SoundPromotedHero"
                | key @ "SoundFallingFromPlane" => {
                    self.set_sound_from_ini(key, trimmed);
                }

                // --- ArmorSet sub-blocks are handled separately ---
                // --- UnitSpecificSounds / UnitSpecificFX sub-blocks are handled separately ---

                // Everything else is silently ignored
                _ => {}
            }
        }
    }
}

impl Default for DefaultThingTemplate {
    fn default() -> Self {
        Self::new("DefaultThing".to_string())
    }
}

impl ThingTemplate for DefaultThingTemplate {
    fn get_name(&self) -> &AsciiString {
        &self.name
    }

    fn get_template_geometry_info(&self) -> GeometryInfo {
        self.geometry_info.clone()
    }

    fn calc_vision_range(&self) -> Real {
        self.vision_range
    }

    fn calc_shroud_clearing_range(&self) -> Real {
        self.shroud_clearing_range
    }

    fn get_command_set_string(&self) -> &AsciiString {
        &self.command_set_string
    }

    fn get_per_unit_sound(&self, name: &str) -> Option<crate::common::audio::AudioEventRts> {
        self.per_unit_sounds.get(name).cloned()
    }

    fn get_voice_attack(&self) -> crate::common::audio::AudioEventRts {
        self.voice_attack.clone()
    }

    fn get_voice_attack_special(&self) -> crate::common::audio::AudioEventRts {
        self.voice_attack_special.clone()
    }

    fn get_voice_attack_air(&self) -> crate::common::audio::AudioEventRts {
        self.voice_attack_air.clone()
    }

    fn is_kind_of(&self, kind: KindOf) -> bool {
        self.kind_of_flags.contains(&kind)
    }

    fn weapon_template_sets(&self) -> &[EngineWeaponTemplateSet] {
        &[]
    }

    fn get_build_cost(&self) -> Int {
        self.build_cost
    }

    fn get_occlusion_delay(&self) -> u32 {
        self.occlusion_delay
    }

    fn calc_cost_to_build(&self, player: Option<&dyn std::any::Any>) -> Int {
        let Some(player) = player.and_then(|p| p.downcast_ref::<crate::player::Player>()) else {
            return self.get_build_cost();
        };

        let mut mods =
            crate::object::production::build_cost_calculator::PlayerBuildModifiers::default();
        mods.production_cost_change_percent =
            player.get_production_cost_change_percent(self.get_name().as_str());
        mods.handicap_cost_multiplier = player
            .get_handicap()
            .get_cost_multiplier_for_template(self);
        mods.production_cost_change_by_kind =
            player.get_production_cost_change_based_on_kind_of(self.kind_of_mask());

        let global_mods =
            crate::object::production::build_cost_calculator::GlobalBuildModifiers::from_global_data();
        let calc =
            crate::object::production::build_cost_calculator::BuildCostCalculator::with_modifiers(
                global_mods,
            );
        calc.calc_cost_to_build(self.get_build_cost(), &mods)
    }

    fn calc_time_to_build(&self, player: Option<&dyn std::any::Any>) -> Int {
        let Some(player) = player.and_then(|p| p.downcast_ref::<crate::player::Player>()) else {
            return (self.get_build_time() * crate::common::LOGICFRAMES_PER_SECOND as f32).round()
                as Int;
        };

        let mut mods =
            crate::object::production::build_cost_calculator::PlayerBuildModifiers::default();
        mods.production_time_change_percent =
            player.get_production_time_change_percent(self.get_name().as_str());
        mods.handicap_time_multiplier = player
            .get_handicap()
            .get_build_time_multiplier_for_template(self);
        mods.energy_supply_ratio = player.get_energy().supply_ratio();
        mods.production_cost_change_by_kind =
            player.get_production_cost_change_based_on_kind_of(self.kind_of_mask());
        #[cfg(any(debug_assertions, feature = "internal", feature = "allow_debug_cheats"))]
        {
            mods.builds_instantly = player.builds_instantly();
        }

        let global_mods =
            crate::object::production::build_cost_calculator::GlobalBuildModifiers::from_global_data();
        let calc =
            crate::object::production::build_cost_calculator::BuildCostCalculator::with_modifiers(
                global_mods,
            );
        calc.calc_time_to_build(self.get_build_time(), &mods, None) as Int
    }

    fn get_build_time(&self) -> Real {
        self.build_time
    }

    fn get_threat_value(&self) -> UnsignedInt {
        self.threat_value
    }

    fn get_crusher_level(&self) -> u32 {
        self.crusher_level
    }

    fn get_crushable_level(&self) -> u32 {
        self.crushable_level
    }

    fn get_raw_transport_slot_count(&self) -> UnsignedByte {
        self.transport_slot_count
    }

    fn get_shroud_reveal_to_all_range(&self) -> Real {
        self.shroud_reveal_to_all_range
    }

    fn get_max_health(&self) -> Real {
        self.max_health
    }

    fn get_energy_production(&self) -> Int {
        self.energy_production
    }

    fn get_energy_bonus(&self) -> Int {
        self.energy_bonus
    }

    fn get_per_unit_fx(&self, name: &str) -> Option<crate::common::audio::AudioEventRts> {
        self.per_unit_fx.get(name).cloned()
    }

    // --- Voice events ---
    fn get_voice_select(&self) -> crate::common::audio::AudioEventRts {
        self.voice_select.clone()
    }

    fn get_voice_group_select(&self) -> crate::common::audio::AudioEventRts {
        self.voice_group_select.clone()
    }

    fn get_voice_move(&self) -> crate::common::audio::AudioEventRts {
        self.voice_move.clone()
    }

    fn get_voice_enter(&self) -> crate::common::audio::AudioEventRts {
        self.voice_enter.clone()
    }

    fn get_voice_fear(&self) -> crate::common::audio::AudioEventRts {
        self.voice_fear.clone()
    }

    fn get_voice_select_elite(&self) -> crate::common::audio::AudioEventRts {
        self.voice_select_elite.clone()
    }

    fn get_voice_created(&self) -> crate::common::audio::AudioEventRts {
        self.voice_created.clone()
    }

    fn get_voice_task_unable(&self) -> crate::common::audio::AudioEventRts {
        self.voice_task_unable.clone()
    }

    fn get_voice_task_complete(&self) -> crate::common::audio::AudioEventRts {
        self.voice_task_complete.clone()
    }

    fn get_voice_meet_enemy(&self) -> crate::common::audio::AudioEventRts {
        self.voice_meet_enemy.clone()
    }

    fn get_voice_garrison(&self) -> crate::common::audio::AudioEventRts {
        self.voice_garrison.clone()
    }

    fn get_voice_defect(&self) -> crate::common::audio::AudioEventRts {
        self.voice_defect.clone()
    }

    fn get_voice_guard(&self) -> crate::common::audio::AudioEventRts {
        self.voice_guard.clone()
    }

    // --- Sound events ---
    fn get_sound_move_start(&self) -> crate::common::audio::AudioEventRts {
        self.sound_move_start.clone()
    }

    fn get_sound_move_start_damaged(&self) -> crate::common::audio::AudioEventRts {
        self.sound_move_start_damaged.clone()
    }

    fn get_sound_move_loop(&self) -> crate::common::audio::AudioEventRts {
        self.sound_move_loop.clone()
    }

    fn get_sound_move_loop_damaged(&self) -> crate::common::audio::AudioEventRts {
        self.sound_move_loop_damaged.clone()
    }

    fn get_sound_stealth_on(&self) -> crate::common::audio::AudioEventRts {
        self.sound_stealth_on.clone()
    }

    fn get_sound_stealth_off(&self) -> crate::common::audio::AudioEventRts {
        self.sound_stealth_off.clone()
    }

    fn get_sound_created(&self) -> crate::common::audio::AudioEventRts {
        self.sound_created.clone()
    }

    fn get_sound_on_damaged(&self) -> crate::common::audio::AudioEventRts {
        self.sound_on_damaged.clone()
    }

    fn get_sound_on_really_damaged(&self) -> crate::common::audio::AudioEventRts {
        self.sound_on_really_damaged.clone()
    }

    fn get_sound_enter(&self) -> crate::common::audio::AudioEventRts {
        self.sound_enter.clone()
    }

    fn get_sound_exit(&self) -> crate::common::audio::AudioEventRts {
        self.sound_exit.clone()
    }

    fn get_sound_promoted_veteran(&self) -> crate::common::audio::AudioEventRts {
        self.sound_promoted_veteran.clone()
    }

    fn get_sound_promoted_elite(&self) -> crate::common::audio::AudioEventRts {
        self.sound_promoted_elite.clone()
    }

    fn get_sound_promoted_hero(&self) -> crate::common::audio::AudioEventRts {
        self.sound_promoted_hero.clone()
    }

    fn get_sound_falling(&self) -> crate::common::audio::AudioEventRts {
        self.sound_falling.clone()
    }
}

// // Implement ThingTemplate for Arc<DefaultThingTemplate> to support Arc-wrapped types
impl ThingTemplate for Arc<DefaultThingTemplate> {
    fn get_name(&self) -> &AsciiString {
        (**self).get_name()
    }

    fn get_template_geometry_info(&self) -> GeometryInfo {
        (**self).get_template_geometry_info()
    }

    fn calc_vision_range(&self) -> Real {
        (**self).calc_vision_range()
    }

    fn calc_shroud_clearing_range(&self) -> Real {
        (**self).calc_shroud_clearing_range()
    }

    fn get_command_set_string(&self) -> &AsciiString {
        (**self).get_command_set_string()
    }

    fn is_kind_of(&self, kind: KindOf) -> bool {
        (**self).is_kind_of(kind)
    }

    fn is_enter_guard(&self) -> bool {
        (**self).is_enter_guard()
    }

    fn is_hijack_guard(&self) -> bool {
        (**self).is_hijack_guard()
    }

    fn weapon_template_sets(&self) -> &[EngineWeaponTemplateSet] {
        (**self).weapon_template_sets()
    }

    fn is_equivalent_to(&self, other: &dyn ThingTemplate) -> bool {
        (**self).is_equivalent_to(other)
    }

    fn get_build_cost(&self) -> Int {
        (**self).get_build_cost()
    }

    fn get_experience_value(&self, level: usize) -> Int {
        (**self).get_experience_value(level)
    }

    fn get_experience_required(&self, level: usize) -> Int {
        (**self).get_experience_required(level)
    }

    fn is_trainable(&self) -> bool {
        (**self).is_trainable()
    }

    fn calc_cost_to_build(&self, player: Option<&dyn std::any::Any>) -> Int {
        (**self).calc_cost_to_build(player)
    }

    fn calc_time_to_build(&self, player: Option<&dyn std::any::Any>) -> Int {
        (**self).calc_time_to_build(player)
    }

    fn get_build_time(&self) -> Real {
        (**self).get_build_time()
    }

    fn get_buildable_status(&self) -> Option<game_engine::common::thing::BuildableStatus> {
        (**self).get_buildable_status()
    }

    fn get_production_prerequisites(&self) -> &[game_engine::common::rts::ProductionPrerequisite] {
        (**self).get_production_prerequisites()
    }

    fn get_max_simultaneous_of_type(&self) -> u32 {
        (**self).get_max_simultaneous_of_type()
    }

    fn get_max_simultaneous_link_key(&self) -> u32 {
        (**self).get_max_simultaneous_link_key()
    }

    fn get_threat_value(&self) -> UnsignedInt {
        (**self).get_threat_value()
    }

    fn get_crusher_level(&self) -> u32 {
        (**self).get_crusher_level()
    }

    fn get_crushable_level(&self) -> u32 {
        (**self).get_crushable_level()
    }

    fn get_raw_transport_slot_count(&self) -> UnsignedByte {
        (**self).get_raw_transport_slot_count()
    }

    fn get_shroud_reveal_to_all_range(&self) -> Real {
        (**self).get_shroud_reveal_to_all_range()
    }

    fn get_occlusion_delay(&self) -> u32 {
        (**self).get_occlusion_delay()
    }

    fn get_energy_production(&self) -> Int {
        (**self).get_energy_production()
    }

    fn get_energy_bonus(&self) -> Int {
        (**self).get_energy_bonus()
    }

    fn structure_rubble_height(&self) -> Option<u8> {
        (**self).structure_rubble_height()
    }

    fn get_per_unit_sound(&self, name: &str) -> Option<crate::common::audio::AudioEventRts> {
        (**self).get_per_unit_sound(name)
    }

    fn get_per_unit_fx(&self, name: &str) -> Option<crate::common::audio::AudioEventRts> {
        (**self).get_per_unit_fx(name)
    }

    fn get_voice_attack(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_attack()
    }

    fn get_voice_attack_special(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_attack_special()
    }

    fn get_voice_attack_air(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_attack_air()
    }

    fn get_voice_select(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_select()
    }

    fn get_voice_group_select(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_group_select()
    }

    fn get_voice_move(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_move()
    }

    fn get_voice_enter(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_enter()
    }

    fn get_voice_fear(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_fear()
    }

    fn get_voice_select_elite(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_select_elite()
    }

    fn get_voice_created(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_created()
    }

    fn get_voice_task_unable(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_task_unable()
    }

    fn get_voice_task_complete(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_task_complete()
    }

    fn get_voice_meet_enemy(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_meet_enemy()
    }

    fn get_voice_garrison(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_garrison()
    }

    fn get_voice_defect(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_defect()
    }

    fn get_voice_guard(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_guard()
    }

    fn get_sound_move_start(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_move_start()
    }

    fn get_sound_move_start_damaged(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_move_start_damaged()
    }

    fn get_sound_move_loop(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_move_loop()
    }

    fn get_sound_move_loop_damaged(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_move_loop_damaged()
    }

    fn get_sound_stealth_on(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_stealth_on()
    }

    fn get_sound_stealth_off(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_stealth_off()
    }

    fn get_sound_created(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_created()
    }

    fn get_sound_on_damaged(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_on_damaged()
    }

    fn get_sound_on_really_damaged(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_on_really_damaged()
    }

    fn get_sound_enter(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_enter()
    }

    fn get_sound_exit(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_exit()
    }

    fn get_sound_promoted_veteran(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_promoted_veteran()
    }

    fn get_sound_promoted_elite(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_promoted_elite()
    }

    fn get_sound_promoted_hero(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_promoted_hero()
    }

    fn get_sound_falling(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_falling()
    }
}

// Implement ThingTemplate for Arc<dyn ThingTemplate> to support trait object Arc wrapping
impl ThingTemplate for Arc<dyn ThingTemplate> {
    fn get_name(&self) -> &AsciiString {
        (**self).get_name()
    }

    fn get_template_geometry_info(&self) -> GeometryInfo {
        (**self).get_template_geometry_info()
    }

    fn calc_vision_range(&self) -> Real {
        (**self).calc_vision_range()
    }

    fn calc_shroud_clearing_range(&self) -> Real {
        (**self).calc_shroud_clearing_range()
    }

    fn is_kind_of(&self, kind: KindOf) -> bool {
        (**self).is_kind_of(kind)
    }

    fn is_enter_guard(&self) -> bool {
        (**self).is_enter_guard()
    }

    fn is_hijack_guard(&self) -> bool {
        (**self).is_hijack_guard()
    }

    fn weapon_template_sets(&self) -> &[EngineWeaponTemplateSet] {
        (**self).weapon_template_sets()
    }

    fn is_equivalent_to(&self, other: &dyn ThingTemplate) -> bool {
        (**self).is_equivalent_to(other)
    }

    fn get_build_cost(&self) -> Int {
        (**self).get_build_cost()
    }

    fn get_experience_value(&self, level: usize) -> Int {
        (**self).get_experience_value(level)
    }

    fn get_experience_required(&self, level: usize) -> Int {
        (**self).get_experience_required(level)
    }

    fn is_trainable(&self) -> bool {
        (**self).is_trainable()
    }

    fn calc_cost_to_build(&self, player: Option<&dyn std::any::Any>) -> Int {
        (**self).calc_cost_to_build(player)
    }

    fn calc_time_to_build(&self, player: Option<&dyn std::any::Any>) -> Int {
        (**self).calc_time_to_build(player)
    }

    fn get_build_time(&self) -> Real {
        (**self).get_build_time()
    }

    fn get_buildable_status(&self) -> Option<game_engine::common::thing::BuildableStatus> {
        (**self).get_buildable_status()
    }

    fn get_production_prerequisites(&self) -> &[game_engine::common::rts::ProductionPrerequisite] {
        (**self).get_production_prerequisites()
    }

    fn get_max_simultaneous_of_type(&self) -> u32 {
        (**self).get_max_simultaneous_of_type()
    }

    fn get_max_simultaneous_link_key(&self) -> u32 {
        (**self).get_max_simultaneous_link_key()
    }

    fn get_threat_value(&self) -> UnsignedInt {
        (**self).get_threat_value()
    }

    fn get_crusher_level(&self) -> u32 {
        (**self).get_crusher_level()
    }

    fn get_crushable_level(&self) -> u32 {
        (**self).get_crushable_level()
    }

    fn get_raw_transport_slot_count(&self) -> UnsignedByte {
        (**self).get_raw_transport_slot_count()
    }

    fn get_shroud_reveal_to_all_range(&self) -> Real {
        (**self).get_shroud_reveal_to_all_range()
    }

    fn get_energy_production(&self) -> Int {
        (**self).get_energy_production()
    }

    fn get_energy_bonus(&self) -> Int {
        (**self).get_energy_bonus()
    }

    fn structure_rubble_height(&self) -> Option<u8> {
        (**self).structure_rubble_height()
    }

    fn get_per_unit_sound(&self, name: &str) -> Option<crate::common::audio::AudioEventRts> {
        (**self).get_per_unit_sound(name)
    }

    fn get_per_unit_fx(&self, name: &str) -> Option<crate::common::audio::AudioEventRts> {
        (**self).get_per_unit_fx(name)
    }

    fn get_voice_attack(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_attack()
    }

    fn get_voice_attack_special(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_attack_special()
    }

    fn get_voice_attack_air(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_attack_air()
    }

    fn get_voice_select(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_select()
    }

    fn get_voice_group_select(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_group_select()
    }

    fn get_voice_move(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_move()
    }

    fn get_voice_enter(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_enter()
    }

    fn get_voice_fear(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_fear()
    }

    fn get_voice_select_elite(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_select_elite()
    }

    fn get_voice_created(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_created()
    }

    fn get_voice_task_unable(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_task_unable()
    }

    fn get_voice_task_complete(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_task_complete()
    }

    fn get_voice_meet_enemy(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_meet_enemy()
    }

    fn get_voice_garrison(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_garrison()
    }

    fn get_voice_defect(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_defect()
    }

    fn get_voice_guard(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_voice_guard()
    }

    fn get_sound_move_start(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_move_start()
    }

    fn get_sound_move_start_damaged(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_move_start_damaged()
    }

    fn get_sound_move_loop(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_move_loop()
    }

    fn get_sound_move_loop_damaged(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_move_loop_damaged()
    }

    fn get_sound_stealth_on(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_stealth_on()
    }

    fn get_sound_stealth_off(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_stealth_off()
    }

    fn get_sound_created(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_created()
    }

    fn get_sound_on_damaged(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_on_damaged()
    }

    fn get_sound_on_really_damaged(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_on_really_damaged()
    }

    fn get_sound_enter(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_enter()
    }

    fn get_sound_exit(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_exit()
    }

    fn get_sound_promoted_veteran(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_promoted_veteran()
    }

    fn get_sound_promoted_elite(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_promoted_elite()
    }

    fn get_sound_promoted_hero(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_promoted_hero()
    }

    fn get_sound_falling(&self) -> crate::common::audio::AudioEventRts {
        (**self).get_sound_falling()
    }
}

