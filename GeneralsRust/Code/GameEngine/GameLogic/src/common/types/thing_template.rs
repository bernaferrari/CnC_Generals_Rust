// Thing / Snapshot / ThingTemplate traits
//
// Split from `types.rs` for module-size parity.
// Observable behavior is unchanged.

// Coordinate helper functions

// Trait definitions for object system interfaces

/// Thing trait (matching C++ Thing base class)
pub trait Thing: Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn get_object_id(&self) -> Option<ObjectID> {
        None
    }
    fn get_template(&self) -> Option<&dyn ThingTemplate>;
    fn get_position(&self) -> &Coord3D;
    fn set_position(&mut self, pos: &Coord3D);
    fn get_angle(&self) -> Real;
    fn set_angle(&mut self, angle: Real);
}

/// Snapshot trait for serialization (matching C++ Snapshot)
pub trait Snapshot {
    fn crc(&self, xfer: &mut dyn Xfer);
    fn xfer(&mut self, xfer: &mut dyn Xfer);
    fn load_post_process(&mut self);
}

/// Thing template interface trait
pub trait ThingTemplate: Any + AsAny + Send + Sync + std::fmt::Debug {
    fn get_name(&self) -> &AsciiString;
    fn get_template_geometry_info(&self) -> GeometryInfo;
    fn get_template_geometry_type(&self) -> Option<EngineGeometryType> {
        None
    }
    fn calc_vision_range(&self) -> Real;
    fn calc_shroud_clearing_range(&self) -> Real;
    fn is_kind_of(&self, kind: KindOf) -> bool;
    fn is_enter_guard(&self) -> bool {
        false
    }
    fn is_hijack_guard(&self) -> bool {
        false
    }
    fn is_build_facility(&self) -> bool {
        false
    }

    /// Get the unique ID for this template
    /// Stub implementation - returns 0 by default
    fn get_id(&self) -> u32 {
        0
    }
    fn weapon_template_sets(&self) -> &[EngineWeaponTemplateSet] {
        &[]
    }
    fn get_build_cost(&self) -> Int {
        0
    }
    /// C++ ThingTemplate::getFenceWidth().
    fn get_fence_width(&self) -> Real {
        0.0
    }
    /// C++ ThingTemplate::getFenceXOffset().
    fn get_fence_x_offset(&self) -> Real {
        0.0
    }
    /// C++ `ThingTemplate::getRawTransportSlotCount` — the parsed
    /// `TransportSlotCount` INI field.
    fn get_raw_transport_slot_count(&self) -> UnsignedByte {
        0
    }
    /// C++ `ThingTemplate::getShadowType` bits (`SHADOW_NONE` = 0).
    fn get_shadow_type_bits(&self) -> u32 {
        0
    }
    /// C++ `ThingTemplate::getShadowSizeX`.
    fn get_shadow_size_x(&self) -> Real {
        0.0
    }
    /// C++ `ThingTemplate::getShadowSizeY`.
    fn get_shadow_size_y(&self) -> Real {
        0.0
    }
    /// C++ `ThingTemplate::getShadowOffsetX`.
    fn get_shadow_offset_x(&self) -> Real {
        0.0
    }
    /// C++ `ThingTemplate::getShadowOffsetY`.
    fn get_shadow_offset_y(&self) -> Real {
        0.0
    }
    /// C++ `ThingTemplate::getShadowTextureName`.
    fn get_shadow_texture_name(&self) -> &str {
        ""
    }

    fn get_experience_value(&self, _level: usize) -> Int {
        0
    }
    fn get_experience_required(&self, _level: usize) -> Int {
        0
    }
    fn is_trainable(&self) -> bool {
        false
    }
    /// Base build time in seconds (matches ThingTemplate::getBuildTime).
    fn get_build_time(&self) -> Real {
        0.0
    }
    /// C++ ThingTemplate::getPlacementViewAngle().
    fn get_placement_view_angle(&self) -> Real {
        0.0
    }
    /// C++ ThingTemplate::getBuildable().
    fn get_buildable_status(&self) -> Option<game_engine::common::thing::BuildableStatus> {
        None
    }
    /// C++ ThingTemplate production prerequisites.
    fn get_production_prerequisites(&self) -> &[game_engine::common::rts::ProductionPrerequisite] {
        &[]
    }
    /// C++ ThingTemplate::getMaxSimultaneousOfType().
    fn get_max_simultaneous_of_type(&self) -> u32 {
        0
    }
    /// C++ ThingTemplate::getMaxSimultaneousLinkKey().
    fn get_max_simultaneous_link_key(&self) -> u32 {
        0
    }
    /// C++ ThingTemplate::getThreatValue().
    fn get_threat_value(&self) -> UnsignedInt {
        0
    }
    /// C++ ThingTemplate::getShroudRevealToAllRange().
    fn get_shroud_reveal_to_all_range(&self) -> Real {
        0.0
    }
    /// Check if this template is equivalent to another template
    fn is_equivalent_to(&self, other: &dyn ThingTemplate) -> bool {
        self.get_name() == other.get_name()
    }

    fn get_initial_object_status(&self) -> ObjectStatusMaskType {
        ObjectStatusMaskType::none()
    }

    fn get_model_name(&self) -> &str {
        self.get_name()
    }

    /// Command set string associated with this template (used by the control bar).
    fn get_command_set_string(&self) -> &AsciiString {
        static EMPTY: OnceLock<AsciiString> = OnceLock::new();
        EMPTY.get_or_init(AsciiString::new)
    }

    fn module_descriptors(&self) -> ModuleDescriptorSet {
        ModuleDescriptorSet::default()
    }

    fn get_draw_module_info(&self) -> &[TemplateModuleInfo] {
        &[]
    }

    fn get_client_update_module_info(&self) -> &[TemplateModuleInfo] {
        &[]
    }

    /// Behavior module descriptors (mirrors C++ ThingTemplate)
    fn get_behavior_module_info(&self) -> &[TemplateModuleInfo] {
        &[]
    }

    /// Maximum health for objects using this template (C++ ThingTemplate::GetMaxHealth)
    fn get_max_health(&self) -> Real {
        0.0
    }

    /// Whether this template supplies physics data
    fn has_physics(&self) -> bool {
        false
    }

    /// Default radar priority for this template.
    /// C++ Reference: ThingTemplate::getDefaultRadarPriority()
    fn get_radar_priority(&self) -> RadarPriorityType {
        RadarPriorityType::Invalid
    }

    /// Crushing power rating for this template.
    /// C++ Reference: ThingTemplate::getCrusherLevel()
    fn get_crusher_level(&self) -> u32 {
        0
    }

    /// Vulnerability to being crushed for this template.
    /// C++ Reference: ThingTemplate::getCrushableLevel()
    fn get_crushable_level(&self) -> u32 {
        255
    }

    /// Initial physics type
    fn get_physics_type(&self) -> PhysicsType {
        PhysicsType::Normal
    }

    /// Mass for physics simulation
    fn get_mass(&self) -> Real {
        0.0
    }

    /// Initial transform for spawned objects
    fn get_initial_transform(&self) -> Matrix3D {
        Matrix3D::IDENTITY
    }

    /// Get occlusion delay in frames.
    /// Returns 0 by default (templates with occlusion data should override).
    fn get_occlusion_delay(&self) -> u32 {
        0
    }

    /// Calculate cost to build with player modifiers.
    /// Uses player modifiers when a Player is supplied.
    fn calc_cost_to_build(&self, player: Option<&dyn std::any::Any>) -> Int {
        let base_cost = self.get_build_cost();
        let Some(player) = player.and_then(|p| p.downcast_ref::<crate::player::Player>()) else {
            return base_cost;
        };

        let mut mods =
            crate::object::production::build_cost_calculator::PlayerBuildModifiers::default();
        mods.production_cost_change_percent =
            player.get_production_cost_change_percent(self.get_name().as_str());
        mods.handicap_cost_multiplier = player
            .get_handicap()
            .get_cost_multiplier_for_template(self);
        mods.production_cost_change_by_kind =
            player.get_production_cost_change_based_on_kind_of(calc_kind_of_mask(self));

        let global_mods =
            crate::object::production::build_cost_calculator::GlobalBuildModifiers::from_global_data();
        let calc =
            crate::object::production::build_cost_calculator::BuildCostCalculator::with_modifiers(
                global_mods,
            );
        calc.calc_cost_to_build(base_cost, &mods)
    }

    /// Energy production/consumption for this template (negative = consumption).
    fn get_energy_production(&self) -> Int {
        0
    }

    /// Extra energy bonus granted by upgrades (e.g., reactor).
    fn get_energy_bonus(&self) -> Int {
        0
    }

    /// Calculate time to build in frames with player modifiers.
    /// Defaults to build time * frames per second, clamped to 0 when no player is supplied.
    fn calc_time_to_build(&self, player: Option<&dyn std::any::Any>) -> Int {
        let base_time = self.get_build_time();
        let Some(player) = player.and_then(|p| p.downcast_ref::<crate::player::Player>()) else {
            let frames = (base_time * crate::common::LOGICFRAMES_PER_SECOND as f32).round() as Int;
            return frames.max(0);
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
            player.get_production_cost_change_based_on_kind_of(calc_kind_of_mask(self));
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
        calc.calc_time_to_build(base_time, &mods, None) as Int
    }

    /// Optional rubble height for structures (0 = use default).
    fn structure_rubble_height(&self) -> Option<u8> {
        None
    }

    /// Per-unit sound lookup (matches ThingTemplate::getPerUnitSound).
    fn get_per_unit_sound(&self, _name: &str) -> Option<crate::common::audio::AudioEventRts> {
        None
    }

    /// Ambient loop sound for the template.
    fn get_sound_ambient(&self) -> Option<crate::common::audio::AudioEventRts> {
        None
    }

    /// Ambient damaged loop sound for the template.
    fn get_sound_ambient_damaged(&self) -> Option<crate::common::audio::AudioEventRts> {
        None
    }

    /// Ambient really-damaged loop sound for the template.
    fn get_sound_ambient_really_damaged(&self) -> Option<crate::common::audio::AudioEventRts> {
        None
    }

    /// Ambient rubble loop sound for the template.
    fn get_sound_ambient_rubble(&self) -> Option<crate::common::audio::AudioEventRts> {
        None
    }

    /// Voice select sound (matches ThingTemplate::getVoiceSelect / TTAUDIO_voiceSelect).
    fn get_voice_select(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice group select sound (matches ThingTemplate::getVoiceGroupSelect / TTAUDIO_voiceGroupSelect).
    fn get_voice_group_select(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice move sound (matches ThingTemplate::getVoiceMove / TTAUDIO_voiceMove).
    fn get_voice_move(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice attack sound (matches ThingTemplate::getVoiceAttack / TTAUDIO_voiceAttack).
    fn get_voice_attack(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice enter sound (matches ThingTemplate::getVoiceEnter / TTAUDIO_voiceEnter).
    fn get_voice_enter(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice fear sound (matches ThingTemplate::getVoiceFear / TTAUDIO_voiceFear).
    fn get_voice_fear(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice select elite sound (matches ThingTemplate::getVoiceSelectElite / TTAUDIO_voiceSelectElite).
    fn get_voice_select_elite(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice created sound (matches ThingTemplate::getVoiceCreated / TTAUDIO_voiceCreated).
    fn get_voice_created(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice task unable sound (matches ThingTemplate::getVoiceTaskUnable / TTAUDIO_voiceTaskUnable).
    fn get_voice_task_unable(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice task complete sound (matches ThingTemplate::getVoiceTaskComplete / TTAUDIO_voiceTaskComplete).
    fn get_voice_task_complete(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice meet enemy sound (matches ThingTemplate::getVoiceMeetEnemy / TTAUDIO_voiceMeetEnemy).
    fn get_voice_meet_enemy(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice garrison sound (matches ThingTemplate::getVoiceGarrison / TTAUDIO_voiceGarrison).
    fn get_voice_garrison(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice defect sound (matches ThingTemplate::getVoiceDefect / TTAUDIO_voiceDefect).
    fn get_voice_defect(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice attack special sound (matches ThingTemplate::getVoiceAttackSpecial / TTAUDIO_voiceAttackSpecial).
    fn get_voice_attack_special(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice attack air sound (matches ThingTemplate::getVoiceAttackAir / TTAUDIO_voiceAttackAir).
    fn get_voice_attack_air(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Voice guard sound (matches ThingTemplate::getVoiceGuard / TTAUDIO_voiceGuard).
    fn get_voice_guard(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Move start sound (matches ThingTemplate::getSoundMoveStart / TTAUDIO_soundMoveStart).
    fn get_sound_move_start(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Move start damaged sound (matches ThingTemplate::getSoundMoveStartDamaged / TTAUDIO_soundMoveStartDamaged).
    fn get_sound_move_start_damaged(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Move loop sound (matches ThingTemplate::getSoundMoveLoop / TTAUDIO_soundMoveLoop).
    fn get_sound_move_loop(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Move loop damaged sound (matches ThingTemplate::getSoundMoveLoopDamaged / TTAUDIO_soundMoveLoopDamaged).
    fn get_sound_move_loop_damaged(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Stealth on sound (matches ThingTemplate::getSoundStealthOn / TTAUDIO_soundStealthOn).
    fn get_sound_stealth_on(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Stealth off sound (matches ThingTemplate::getSoundStealthOff / TTAUDIO_soundStealthOff).
    fn get_sound_stealth_off(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Sound created (matches ThingTemplate::getSoundCreated / TTAUDIO_soundCreated).
    fn get_sound_created(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Sound on damaged (matches ThingTemplate::getSoundOnDamaged / TTAUDIO_soundOnDamaged).
    fn get_sound_on_damaged(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Sound on really damaged (matches ThingTemplate::getSoundOnReallyDamaged / TTAUDIO_soundOnReallyDamaged).
    fn get_sound_on_really_damaged(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Sound enter (matches ThingTemplate::getSoundEnter / TTAUDIO_soundEnter).
    fn get_sound_enter(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Sound exit (matches ThingTemplate::getSoundExit / TTAUDIO_soundExit).
    fn get_sound_exit(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Sound promoted veteran (matches ThingTemplate::getSoundPromotedVeteran / TTAUDIO_soundPromotedVeteran).
    fn get_sound_promoted_veteran(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Sound promoted elite (matches ThingTemplate::getSoundPromotedElite / TTAUDIO_soundPromotedElite).
    fn get_sound_promoted_elite(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Sound promoted hero (matches ThingTemplate::getSoundPromotedHero / TTAUDIO_soundPromotedHero).
    fn get_sound_promoted_hero(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Sound falling from plane (matches ThingTemplate::getSoundFalling / TTAUDIO_soundFalling).
    fn get_sound_falling(&self) -> crate::common::audio::AudioEventRts {
        crate::common::audio::AudioEventRts::default()
    }

    /// Per-unit FX lookup (matches ThingTemplate::getPerUnitFX).
    fn get_per_unit_fx(&self, _name: &str) -> Option<crate::common::audio::AudioEventRts> {
        None
    }
}

fn calc_kind_of_mask<T: ThingTemplate + ?Sized>(template: &T) -> KindOfMaskType {
    let mut mask: KindOfMaskType = KIND_OF_MASK_NONE;
        for &kind in ALL_KIND_OF {
            if template.is_kind_of(kind) {
                mask |= kind.cpp_mask();
            }
    }
    mask
}

