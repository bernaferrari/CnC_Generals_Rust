#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

/// C++ `TheGameText->fetch("GUI:MaxSelectionSize").format(count)`.
fn format_max_selection_size_message(max: i32) -> String {
    #[cfg(feature = "game_client")]
    {
        let template = game_client::game_text::GameText::fetch("GUI:MaxSelectionSize");
        if template.contains("%d") || template.contains("%i") {
            return template
                .replace("%d", &max.to_string())
                .replace("%i", &max.to_string());
        }
        if !template.is_empty() && !template.starts_with("MISSING:") {
            return template;
        }
    }
    format!("You cannot select more than {max} units.")
}

/// Convert a still-armed FIRE_WEAPON button into Main's authoritative order.
///
/// `ATTACK_OBJECTS_POSITION` deliberately keeps the target object only for
/// click validation.  C++ emits `MSG_DO_WEAPON_AT_LOCATION` in that case, so
/// the host must use the click's terrain position even when the picker also
/// supplied an object ID.
fn resolve_pending_weapon_command(
    weapon: PendingWeaponCommand,
    location: glam::Vec3,
    target_object: Option<crate::game_logic::ObjectId>,
) -> crate::command_system::CommandType {
    let target = match target_object {
        Some(_) if weapon.attacks_object_position() => {
            crate::command_system::WeaponTarget::Location(location)
        }
        Some(target_id) => crate::command_system::WeaponTarget::Object(target_id),
        None => crate::command_system::WeaponTarget::Location(location),
    };
    crate::command_system::CommandType::DoWeapon {
        weapon_slot: weapon.weapon_slot,
        max_shots_to_fire: weapon.max_shots_to_fire,
        target,
    }
}

/// Convert an armed `COMBATDROP` button using C++'s target precedence.
///
/// `CommandXlat::issueCombatDropCommand` emits `MSG_COMBATDROP_AT_OBJECT`
/// when an object was clicked and the button has any
/// `COMMAND_OPTION_NEED_OBJECT_TARGET` bit.  It only falls through to the
/// terrain location when that object route is unavailable and `NEED_TARGET_POS`
/// is present.
fn resolve_pending_combat_drop_command(
    combat_drop: PendingCombatDropCommand,
    location: glam::Vec3,
    target_object: Option<crate::game_logic::ObjectId>,
) -> Option<crate::command_system::CommandType> {
    if let Some(target_id) = target_object.filter(|_| combat_drop.accepts_object_target()) {
        return Some(crate::command_system::CommandType::CombatDrop {
            target: crate::command_system::DropTarget::Object(target_id),
        });
    }
    combat_drop.accepts_position_target().then_some(
        crate::command_system::CommandType::CombatDrop {
            target: crate::command_system::DropTarget::Location(location),
        },
    )
}

/// C++ PlaceEventTranslator illegal place: VoiceNoBuild + NoCanDoSound.
fn play_host_illegal_place_feedback(
    engine: &CnCGameEngine,
    builder_id: crate::game_logic::ObjectId,
) {
    let template = engine
        .presentation_ro(builder_id)
        .map(|o| o.template_name.clone())
        .or_else(|| {
            engine
                .game_logic
                .host_object(builder_id)
                .map(|o| o.template_name.clone())
        });
    if let Some(name) = template.as_deref().and_then(resolve_voice_no_build) {
        let _ = crate::assets::audio::play_sound_through_the_audio(&name);
    }
    let _ = crate::assets::audio::play_sound_through_the_audio("NoCanDoSound");
}

fn resolve_voice_no_build(template_name: &str) -> Option<String> {
    let guard = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
    let factory = guard.as_ref()?;
    let tmpl = factory.find_template(template_name, false)?;
    let event = tmpl.get_per_unit_sound(&"VoiceNoBuild".to_string())?;
    let name = event.get_event_name();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// Leftover SpecialPower.ini `RadiusCursorRadius` by template name.
fn leftover_store_radius_cursor_radius(template_name: &str) -> f32 {
    if let Some(store) = gamelogic::object::special_power_template::get_special_power_store() {
        if let Some(template) = store.find_special_power_template(template_name) {
            let radius = template.get_radius_cursor_radius();
            if radius > 0.0 {
                return radius;
            }
        }
    }
    crate::game_logic::host_sp_science_upgrade_player_team_residual_wave109::special_power_template_row_wave109(
        template_name,
    )
    .map(|row| row.radius_cursor_radius)
    .filter(|r| *r > 0.0)
    .unwrap_or(0.0)
}

fn leftover_special_templates_for_cursor(cursor_type: &str) -> &'static [&'static str] {
    match cursor_type {
        "DAISYCUTTER" => &["SuperweaponDaisyCutter"],
        "A10STRIKE" => &["SuperweaponA10ThunderboltMissileStrike"],
        "SCUDSTORM" => &["SuperweaponScudStorm"],
        "PARTICLECANNON" => &["SuperweaponParticleUplinkCannon"],
        "SPYSATELLITE" => &["SpecialPowerSpySatellite"],
        "RADAR" => &["SpecialPowerRadarVanScan"],
        "CARPETBOMB" => &["SuperweaponCarpetBomb"],
        "CLUSTERMINES" => &["SuperweaponClusterMines"],
        "PARADROP" => &["SuperweaponParadropAmerica"],
        "SPYDRONE" => &["SpecialPowerSpyDrone"],
        "NUCLEARMISSILE" => &["SuperweaponNeutronMissile", "SuperweaponNuclearMissile"],
        "EMPPULSE" => &["SuperweaponEMPPulse"],
        "ARTILLERYBARRAGE" => &["SuperweaponArtilleryBarrage"],
        "NAPALMSTRIKE" => &["SuperweaponNapalmStrike"],
        "SPECTREGUNSHIP" => &["SuperweaponSpectreGunship"],
        "ANTHRAXBOMB" => &["SuperweaponAnthraxBomb"],
        "AMBUSH" => &["SuperweaponRebelAmbush"],
        "FRENZY" => &["SpecialPowerFrenzy"],
        "EMERGENCY_REPAIR" => &["SpecialPowerEmergencyRepair"],
        "HELIX_NAPALM_BOMB" => &["HelixNapalmBomb"],
        "AMBULANCE" => &["SpecialPowerAmbulance"],
        _ => &[],
    }
}

fn leftover_store_radius_cursor_for_type(cursor_type: &str) -> f32 {
    for name in leftover_special_templates_for_cursor(cursor_type) {
        let radius = leftover_store_radius_cursor_radius(name);
        if radius > 0.0 {
            return radius;
        }
    }
    if cursor_type == "OFFENSIVE_SPECIALPOWER" {
        return 0.0;
    }
    crate::ui::construction_panel::RadiusCursorOverlay::radius_for_type(cursor_type)
}

fn leftover_special_template_name_for_power(
    power: &crate::command_system::SpecialPowerType,
) -> Option<&'static str> {
    leftover_special_templates_for_cursor(CnCGameEngine::radius_cursor_type_for_special_power(
        power,
    ))
    .first()
    .copied()
}

/// Whether a shared ControlBar superweapon button can arm this exact parsed
/// module power.
///
/// Retail faction variants intentionally share a Particle Uplink or Nuclear
/// Missile button/cursor family, but the module's `SpecialPowerTemplate`
/// remains the execution authority.  This only joins the three documented
/// variants in each family; it must never turn a template/object name into a
/// special-power capability.
fn parsed_structure_superweapon_matches_button(
    requested: &crate::command_system::SpecialPowerType,
    parsed: &crate::command_system::SpecialPowerType,
) -> bool {
    use crate::command_system::SpecialPowerType as Power;

    matches!(
        (requested, parsed),
        (
            Power::ParticleCannon | Power::SuperweaponParticleCannon | Power::LaserCannon,
            Power::ParticleCannon | Power::SuperweaponParticleCannon | Power::LaserCannon,
        ) | (
            Power::NuclearMissile | Power::NukeNeutronMissile | Power::SuperweaponNeutronMissile,
            Power::NuclearMissile | Power::NukeNeutronMissile | Power::SuperweaponNeutronMissile,
        )
    ) || requested == parsed
}

/// Return the exact parsed module power after the presentation-only button
/// family check.  The returned value, rather than the generic button's
/// baseline enum, is placed in the pending map command.
fn exact_parsed_structure_power_for_button(
    requested: &crate::command_system::SpecialPowerType,
    parsed: crate::command_system::SpecialPowerType,
) -> Option<crate::command_system::SpecialPowerType> {
    parsed_structure_superweapon_matches_button(requested, &parsed).then_some(parsed)
}

impl CnCGameEngine {
    /// C++ `TheInGameUI->getGUICommand()` option bits, or `None` when no
    /// GUI command is armed (force-attack then uses current-selection pick).
    pub(super) fn host_armed_gui_command_options(&self) -> Option<u32> {
        if let Some(kind) = self.pending_map_command.as_ref() {
            return Some(kind.command_option_bits().unwrap_or(0));
        }
        #[cfg(feature = "game_client")]
        {
            if let Some(pending) = game_client::helpers::TheInGameUI::get_pending_command() {
                return Some(pending.options);
            }
            if let Some(pending) = game_client::helpers::TheInGameUI::get_pending_special_power() {
                return Some(pending.options);
            }
        }
        None
    }

    /// Prime the immutable host-side barrel-topology catalogue for every
    /// Object template actually present in a freshly loaded world.
    ///
    /// C++ asks each live Drawable for its current W3DModelDraw barrel count
    /// immediately before a shot. Main keeps GameLogic and WGPU ownership
    /// separate, so models are allowed to enter the shared AssetManager only
    /// at this successful world boundary; simulation then uses a cache-only
    /// query keyed by the current exact ModelCondition state. We prewarm all
    /// finite source Condition/Transition states for active templates so a
    /// later FIRING/DAMAGED/upgrade transition never opens an archive from a
    /// fixed combat step.
    fn prewarm_host_weapon_barrel_topologies_for_loaded_world(&mut self) {
        let template_names: Vec<String> = self
            .game_logic
            .host_objects()
            .values()
            .map(|object| object.template_name.trim())
            .filter(|name| !name.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        if template_names.is_empty() {
            return;
        }
        let active_selections: Vec<(String, u128)> = self
            .game_logic
            .host_objects()
            .values()
            .filter_map(|object| {
                let name = object.template_name.trim();
                (!name.is_empty()).then(|| (name.to_string(), object.model_condition_bits))
            })
            .collect();

        let Some(manager_arc) = crate::assets::get_asset_manager() else {
            return;
        };
        let Ok(mut manager) = manager_arc.lock() else {
            warn!("Weapon barrel topology prewarm skipped: asset manager mutex poisoned");
            return;
        };

        let source_stats =
            manager.prewarm_weapon_barrel_topology_models_for_objects(template_names.iter());
        let active_stats =
            manager.prewarm_weapon_barrel_topologies_for_object_conditions(active_selections);
        if source_stats.requested != 0 || active_stats.requested != 0 {
            debug!(
                "Prewarmed host W3D barrel topology: source requested={} hits={} resolved={} missing={}; active requested={} hits={} resolved={} missing={}",
                source_stats.requested,
                source_stats.cache_hits,
                source_stats.resolved,
                source_stats.missing,
                active_stats.requested,
                active_stats.cache_hits,
                active_stats.resolved,
                active_stats.missing,
            );
        }
    }

    pub(super) fn commit_pending_map_command(
        &mut self,
        location: glam::Vec3,
        target_object: Option<crate::game_logic::ObjectId>,
    ) {
        let Some(kind) = self.pending_map_command.clone() else {
            return;
        };
        let player_id = self.current_player_id;
        // Wave 219: selection via presentation-first ui_selected_ids.
        let selected = self.ui_selected_ids(player_id);
        let allows_empty = matches!(kind, PendingMapCommand::PlaceBeacon);
        if selected.is_empty() && !allows_empty {
            return;
        }
        // C++ CommandXlat.cpp:1505-1629 — NEED_TARGET_* relationship nulls the
        // object; invalid DO_COMMAND leaves the GUI command armed.
        let filtered_target = self.filter_pending_map_target(&kind, target_object);
        if !self.pending_map_command_currently_valid(&kind, filtered_target) {
            // C++ CommandXlat.cpp:1505-1629 — invalid DO_COMMAND stays armed.
            return;
        }
        let _ = self.pending_map_command.take();
        // C++ GUICommandTranslator.cpp:471-473: keep selection for one
        // alternate-mouse blank LMB after a completed non-context GUI command.
        self.host_set_prevent_left_click_deselection(true);

        self.clear_radius_cursor_overlays();
        let command_type = match kind {
            PendingMapCommand::AttackMove => crate::command_system::CommandType::AttackMoveTo {
                destination: location,
                max_shots: -1,
            },
            PendingMapCommand::Guard(mode) => {
                if let Some(tid) = filtered_target {
                    crate::command_system::CommandType::Guard {
                        target: crate::command_system::GuardTarget::Object(tid),
                        mode,
                    }
                } else {
                    crate::command_system::CommandType::Guard {
                        target: crate::command_system::GuardTarget::Position(location),
                        mode,
                    }
                }
            }
            PendingMapCommand::SetRallyPoint => {
                crate::command_system::CommandType::SetRallyPoint { location }
            }
            PendingMapCommand::CombatDrop(combat_drop) => {
                let Some(command) = resolve_pending_combat_drop_command(
                    combat_drop.clone(),
                    location,
                    filtered_target,
                ) else {
                    self.pending_map_command = Some(PendingMapCommand::CombatDrop(combat_drop));
                    return;
                };
                command
            }
            PendingMapCommand::SpecialPower(power_type) => {
                let target = if let Some(tid) = filtered_target {
                    crate::command_system::PowerTarget::Object(tid)
                } else {
                    crate::command_system::PowerTarget::Location(location)
                };
                crate::command_system::CommandType::DoSpecialPower { power_type, target }
            }
            PendingMapCommand::Weapon(weapon) => {
                resolve_pending_weapon_command(weapon, location, filtered_target)
            }
            PendingMapCommand::PlaceBeacon => crate::command_system::CommandType::PlaceBeacon {
                location,
                text: String::new(),
            },
            PendingMapCommand::UnitAbility(ability) => {
                let Some(tid) = filtered_target else {
                    self.pending_map_command = Some(PendingMapCommand::UnitAbility(ability));
                    return;
                };
                match ability {
                    PendingUnitAbility::Hijack => {
                        crate::command_system::CommandType::Hijack { target_id: tid }
                    }
                    PendingUnitAbility::Sabotage => {
                        crate::command_system::CommandType::Sabotage { target_id: tid }
                    }
                    PendingUnitAbility::CaptureBuilding => {
                        crate::command_system::CommandType::CaptureBuilding { target_id: tid }
                    }
                    PendingUnitAbility::SnipeVehicle => {
                        crate::command_system::CommandType::SnipeVehicle { target_id: tid }
                    }
                    PendingUnitAbility::PlantTimedDemoCharge => {
                        crate::command_system::CommandType::PlantTimedDemoCharge { target_id: tid }
                    }
                    PendingUnitAbility::PlantRemoteDemoCharge => {
                        crate::command_system::CommandType::PlantRemoteDemoCharge { target_id: tid }
                    }
                    PendingUnitAbility::StealCashHack => {
                        crate::command_system::CommandType::StealCashHack { target_id: tid }
                    }
                    PendingUnitAbility::DisableVehicleHack => {
                        crate::command_system::CommandType::DisableVehicleHack { target_id: tid }
                    }
                    PendingUnitAbility::HackerDisableBuilding => {
                        crate::command_system::CommandType::HackerDisableBuilding { target_id: tid }
                    }
                    PendingUnitAbility::DisguiseAsVehicle => {
                        crate::command_system::CommandType::DisguiseAsVehicle { target_id: tid }
                    }
                    PendingUnitAbility::PlantBoobyTrap => {
                        crate::command_system::CommandType::PlantBoobyTrap { target_id: tid }
                    }
                    PendingUnitAbility::ConvertToCarbomb => {
                        crate::command_system::CommandType::ConvertToCarbomb { target_id: tid }
                    }
                    PendingUnitAbility::Repair => {
                        crate::command_system::CommandType::Repair { target_id: tid }
                    }
                }
            }
        };
        self.host_queue_and_process_command_silent(crate::command_system::GameCommand {
            command_type,
            player_id,
            command_id: 0,
            timestamp: std::time::SystemTime::now(),
            selected_units: selected,
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        self.host_process_commands_with_command_sound();
    }

    const NEED_TARGET_ENEMY_OBJECT: u32 = 0x0000_0001;
    const NEED_TARGET_NEUTRAL_OBJECT: u32 = 0x0000_0002;
    const NEED_TARGET_ALLY_OBJECT: u32 = 0x0000_0004;
    const NEED_TARGET_POS: u32 = 0x0000_0020;
    const NEED_OBJECT_TARGET: u32 = Self::NEED_TARGET_ENEMY_OBJECT
        | Self::NEED_TARGET_NEUTRAL_OBJECT
        | Self::NEED_TARGET_ALLY_OBJECT;

    fn filter_pending_map_target(
        &self,
        kind: &PendingMapCommand,
        target_object: Option<crate::game_logic::ObjectId>,
    ) -> Option<crate::game_logic::ObjectId> {
        let Some(tid) = target_object else {
            return None;
        };
        let options = self.pending_command_option_bits(kind);
        if options & Self::NEED_OBJECT_TARGET == 0 {
            return Some(tid);
        }
        if self.pending_target_relationship_allowed(options, tid) {
            Some(tid)
        } else {
            None
        }
    }

    fn pending_command_option_bits(&self, kind: &PendingMapCommand) -> u32 {
        if let Some(bits) = kind.command_option_bits() {
            return bits;
        }
        match kind {
            PendingMapCommand::SpecialPower(power) => special_power_pending_options(power),
            PendingMapCommand::UnitAbility(_) => {
                Self::NEED_TARGET_ENEMY_OBJECT
                    | Self::NEED_TARGET_NEUTRAL_OBJECT
                    | Self::NEED_TARGET_ALLY_OBJECT
            }
            _ => 0,
        }
    }

    fn pending_target_relationship_allowed(
        &self,
        options: u32,
        target: crate::game_logic::ObjectId,
    ) -> bool {
        let needs_enemy = options & Self::NEED_TARGET_ENEMY_OBJECT != 0;
        let needs_neutral = options & Self::NEED_TARGET_NEUTRAL_OBJECT != 0;
        let needs_ally = options & Self::NEED_TARGET_ALLY_OBJECT != 0;
        if !(needs_enemy || needs_neutral || needs_ally) {
            return true;
        }
        if let Some(hint) = self.presentation_target_hint(target) {
            if needs_enemy && hint.is_enemy_of_local {
                return true;
            }
            if needs_ally && hint.is_friendly_of_local {
                return true;
            }
            if needs_neutral && hint.is_neutral {
                return true;
            }
            return false;
        }
        if let Some(obj) = self.game_logic.host_object(target) {
            let local_team = self
                .last_presentation_frame
                .as_ref()
                .map(|f| f.local_team)
                .or(self.host_match_local_team)
                .unwrap_or(obj.team);
            if needs_ally && obj.team == local_team {
                return true;
            }
            if needs_neutral && obj.team == crate::game_logic::Team::Neutral {
                return true;
            }
            if needs_enemy && obj.team != local_team && obj.team != crate::game_logic::Team::Neutral
            {
                return true;
            }
        }
        false
    }

    fn pending_map_command_currently_valid(
        &self,
        kind: &PendingMapCommand,
        filtered_target: Option<crate::game_logic::ObjectId>,
    ) -> bool {
        let options = self.pending_command_option_bits(kind);
        let needs_object = options & Self::NEED_OBJECT_TARGET != 0;
        let needs_pos = options & Self::NEED_TARGET_POS != 0;
        match kind {
            PendingMapCommand::UnitAbility(_) => filtered_target.is_some(),
            PendingMapCommand::SpecialPower(_) | PendingMapCommand::Weapon(_) => {
                if needs_object && !needs_pos {
                    filtered_target.is_some()
                } else {
                    true
                }
            }
            _ => true,
        }
    }

    pub(super) fn host_command_xlat_multiplayer_meta(&self) -> bool {
        self.host_is_in_multiplayer_game() && !self.presentation_or_boot_in_replay_game()
    }

    pub(super) fn cancel_structure_placement_from_ui(&mut self) {
        self.pending_structure_placement = None;
        self.game_hud.construction_panel.clear_structure_placement();
        self.ui_manager
            .game_hud_mut()
            .construction_panel
            .clear_structure_placement();
        game_client::helpers::TheInGameUI::place_build_available(None, None);
        game_client::helpers::TheInGameUI::clear_pending_special_power();
        game_client::helpers::TheInGameUI::set_placement_start(None);
    }

    /// Update structure placement ghost legality under cursor residual.

    pub(super) fn radius_cursor_type_for_special_power(
        power: &crate::command_system::SpecialPowerType,
    ) -> &'static str {
        use crate::command_system::SpecialPowerType as P;
        match power {
            P::ParticleCannon | P::SuperweaponParticleCannon | P::LaserCannon => "PARTICLECANNON",
            P::NuclearMissile
            | P::NukeNeutronMissile
            | P::SuperweaponNeutronMissile
            | P::BlackMarketNuke
            | P::DetonateDirtyNuke => "NUCLEARMISSILE",
            P::ScudStorm => "SCUDSTORM",
            P::Airstrike => "A10STRIKE",
            P::CarpetBomb | P::EarlyChinaCarpetBomb | P::AirForceCarpetBomb => "CARPETBOMB",
            P::DaisyCutter | P::FuelAirBomb => "DAISYCUTTER",
            P::Paradrop | P::InfantryParadrop | P::TankParadrop => "PARADROP",
            P::NapalmStrike => "NAPALMSTRIKE",
            P::Artillery => "ARTILLERYBARRAGE",
            P::EmpPulse => "EMPPULSE",
            P::SpectreGunship => "SPECTREGUNSHIP",
            P::SpySatellite | P::CiaIntelligence => "SPYSATELLITE",
            P::ClusterMines => "CLUSTERMINES",
            P::Ambush | P::TerrorCell => "AMBUSH",
            P::Frenzy | P::EarlyFrenzy => "FRENZY",
            P::AnthraxBomb => "ANTHRAXBOMB",
            P::EmergencyRepair | P::EarlyEmergencyRepair => "EMERGENCY_REPAIR",
            P::SpyDrone => "SPYDRONE",
            P::RadarScan => "RADAR",
            _ => "OFFENSIVE_SPECIALPOWER",
        }
    }

    /// C++ `InGameUI::setRadiusCursor` radius (InGameUI.cpp:1210-1258).
    /// Leftover `resolve_radius_cursor_radius`: ATTACK_DAMAGE_AREA = primary
    /// damage, SCATTER = scatter+scalar, CONTINUE/CLEARMINES = continueAttackRange,
    /// GUARD = `AIGuardMachine::get_std_guard_range`, special = RadiusCursorRadius.
    /// Never uses the construction_panel OFFENSIVE_SPECIALPOWER=0 table.
    pub(super) fn resolve_radius_cursor_radius(&self, cursor_type: &str) -> f32 {
        let seed = self.ui_selection_seed_id();

        #[cfg(feature = "game_client")]
        {
            if let Some(kind) = game_client::in_game_ui::RadiusCursorType::from_name(cursor_type) {
                let leftover =
                    game_client::in_game_ui::InGameUI::leftover_resolve_radius_cursor_radius(
                        kind,
                        seed.map(|id| id.0),
                        0.0,
                    );
                if leftover > 0.0 {
                    return leftover;
                }
            }
        }

        let weapon_name = self.radius_cursor_primary_weapon_name(seed);
        match cursor_type {
            "ATTACK_DAMAGE_AREA" => self.radius_cursor_primary_damage(weapon_name.as_deref()),
            "ATTACK_SCATTER_AREA" => self.radius_cursor_scatter(weapon_name.as_deref()),
            "ATTACK_CONTINUE_AREA" | "CLEARMINES" => {
                self.radius_cursor_continue_attack(weapon_name.as_deref())
            }
            "GUARD_AREA" => self.radius_cursor_guard_range(seed),
            _ => {
                let special = self.leftover_special_power_radius_cursor(cursor_type);
                if special > 0.0 { special } else { 0.0 }
            }
        }
    }

    fn radius_cursor_primary_weapon_name(
        &self,
        seed: Option<crate::game_logic::ObjectId>,
    ) -> Option<String> {
        let id = seed?;
        if let Some(obj) = self.game_logic.host_object(id) {
            if let Some(name) = obj.weapon_name_for_slot(0) {
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
            if let Some(name) = obj.get_template().primary_weapon_name.as_deref() {
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
            let template = obj.template_name.clone();
            if let Some(name) = crate::game_logic::primary_weapon_name_for_unit(&template) {
                return Some(name.to_string());
            }
            if !template.is_empty() {
                return Some(template);
            }
        }
        let template = self
            .last_presentation_frame
            .as_ref()
            .and_then(|frame| frame.objects.iter().find(|o| o.id == id))
            .map(|o| o.template_name.as_str())
            .unwrap_or("");
        crate::game_logic::primary_weapon_name_for_unit(template)
            .map(str::to_string)
            .or_else(|| {
                if template.is_empty() {
                    None
                } else {
                    Some(template.to_string())
                }
            })
    }

    fn radius_cursor_primary_damage(&self, weapon_name: Option<&str>) -> f32 {
        let Some(name) = weapon_name.filter(|n| !n.is_empty()) else {
            return 0.0;
        };
        crate::game_logic::weapon_bootstrap::host_primary_damage_radius_for_weapon_name(name)
    }

    fn radius_cursor_scatter(&self, weapon_name: Option<&str>) -> f32 {
        let Some(name) = weapon_name.filter(|n| !n.is_empty()) else {
            return 0.0;
        };
        crate::game_logic::weapon_bootstrap::host_scatter_radius_for_weapon_name(name)
            + crate::game_logic::weapon_bootstrap::host_scatter_target_scalar_for_weapon_name(name)
    }

    fn radius_cursor_continue_attack(&self, weapon_name: Option<&str>) -> f32 {
        let Some(name) = weapon_name.filter(|n| !n.is_empty()) else {
            return 0.0;
        };
        crate::game_logic::weapon_bootstrap::host_continue_attack_range_for_weapon_name(name)
    }

    fn radius_cursor_guard_range(&self, seed: Option<crate::game_logic::ObjectId>) -> f32 {
        let Some(id) = seed else {
            return 0.0;
        };
        let leftover = gamelogic::ai::guard::AIGuardMachine::get_std_guard_range(id.0);
        if leftover > 0.0 {
            if leftover > 100.0
                || gamelogic::object::registry::OBJECT_REGISTRY
                    .get_object(id.0)
                    .is_some()
            {
                return leftover;
            }
        }
        let (inner, _) = self.game_logic.host_std_guard_ranges(id);
        if inner > 0.0 {
            inner
        } else if leftover > 0.0 {
            leftover
        } else {
            0.0
        }
    }

    fn leftover_special_power_radius_cursor(&self, cursor_type: &str) -> f32 {
        #[cfg(feature = "game_client")]
        {
            if let Some(pending) = game_client::helpers::TheInGameUI::get_pending_special_power() {
                if let Some(store) =
                    gamelogic::object::special_power_template::get_special_power_store()
                {
                    if let Some(template) =
                        store.find_special_power_template_by_id(pending.power_id)
                    {
                        let radius = template.get_radius_cursor_radius();
                        if radius > 0.0 {
                            return radius;
                        }
                    }
                }
            }
        }
        if let Some(PendingMapCommand::SpecialPower(power)) = self.pending_map_command.as_ref() {
            if let Some(name) = leftover_special_template_name_for_power(power) {
                let radius = leftover_store_radius_cursor_radius(name);
                if radius > 0.0 {
                    return radius;
                }
            }
        }
        leftover_store_radius_cursor_for_type(cursor_type)
    }

    pub(super) fn arm_radius_cursor_for_pending(&mut self, cursor_type: &str) {
        use crate::ui::construction_panel::RadiusCursorOverlay;
        let r = self.resolve_radius_cursor_radius(cursor_type);
        if r <= 0.0 {
            return;
        }
        let mut ov = RadiusCursorOverlay::new(cursor_type, r);
        let loc = self.mouse_world_position;
        ov.centre = (loc.x, loc.z);
        self.game_hud
            .construction_panel
            .set_radius_overlay(Some(ov.clone()));
        self.ui_manager
            .game_hud_mut()
            .construction_panel
            .set_radius_overlay(Some(ov));
    }

    pub(super) fn clear_radius_cursor_overlays(&mut self) {
        self.game_hud.construction_panel.clear_radius_overlay();
        self.ui_manager
            .game_hud_mut()
            .construction_panel
            .clear_radius_overlay();
    }

    pub(super) fn sync_pending_map_command_radius_cursor(&mut self) {
        let Some(kind) = self.pending_map_command.clone() else {
            // Keep structure placement path separate; clear only if no pending map cmd.
            return;
        };
        let cursor = match kind {
            PendingMapCommand::AttackMove => "ATTACK_CONTINUE_AREA",
            PendingMapCommand::Guard(_) => "GUARD_AREA",
            PendingMapCommand::SetRallyPoint => "FRIENDLY_SPECIALPOWER",
            PendingMapCommand::CombatDrop(_) => "COMBATDROP",
            PendingMapCommand::PlaceBeacon => "RADAR",
            PendingMapCommand::SpecialPower(ref p) => Self::radius_cursor_type_for_special_power(p),
            PendingMapCommand::Weapon(ref weapon) if weapon.uses_mine_clearing_weapon_set() => {
                "CLEARMINES"
            }
            PendingMapCommand::Weapon(_) => "ATTACK_DAMAGE_AREA",
            PendingMapCommand::UnitAbility(_) => "OFFENSIVE_SPECIALPOWER",
        };
        // Ensure overlay exists (re-arm if missing).
        if self.game_hud.construction_panel.radius_overlay().is_none() {
            self.arm_radius_cursor_for_pending(cursor);
        }
        let loc = self.mouse_world_position;
        self.game_hud
            .construction_panel
            .sync_radius_overlay_cursor(loc.x, loc.z);
        self.ui_manager
            .game_hud_mut()
            .construction_panel
            .sync_radius_overlay_cursor(loc.x, loc.z);
    }

    pub(super) fn sync_pending_structure_placement_cursor(&mut self) {
        let Some(template) = self.pending_structure_placement.clone() else {
            return;
        };
        let loc = self.mouse_world_position;
        // Wave 220: team via presentation-first local_team_for_ui.
        let team = self.local_team_for_ui();
        // Wave 219: builder identity via presentation-first ui_selected_ids.
        let builder_id = self
            .ui_selected_ids(self.current_player_id)
            .first()
            .copied();
        // Wave 924: placement cursor uses host legal-build residual cache (no live dual-read).
        // Preview: IGNORE_STEALTHED so unseen stealthed units do not redden the ghost.
        let code = self.host_legal_build_code_at_for_preview(team, loc, &template, builder_id);
        let legal = code == crate::game_logic::host_production_buildable_command_residual::LBC_OK;
        // Dual HUD residual
        self.game_hud
            .construction_panel
            .sync_structure_placement_cursor(loc.x, loc.z, legal);
        self.ui_manager
            .game_hud_mut()
            .construction_panel
            .sync_structure_placement_cursor(loc.x, loc.z, legal);
    }

    pub(super) fn begin_structure_placement_from_ui(&mut self, template_name: &str) {
        if template_name.trim().is_empty() {
            return;
        }
        self.pending_structure_placement = Some(template_name.to_string());
        // Dual HUD residual: engine HUD + interactive UIManager HUD ghosts.
        self.game_hud
            .construction_panel
            .arm_structure_placement(template_name.to_string());
        self.ui_manager
            .game_hud_mut()
            .construction_panel
            .arm_structure_placement(template_name.to_string());
        // C++ ControlBar::enterTargetingMode / placeBuildAvailable stores the
        // selected dozer as the pending place source. LMB-down cancels if gone.
        let selected = self.ui_selected_ids(self.current_player_id);
        let source_id = selected
            .iter()
            .copied()
            .find(|&id| self.ui_object_is_dozer(id))
            .or_else(|| selected.first().copied())
            .map(|id| id.0)
            .or_else(|| {
                let existing =
                    game_client::helpers::TheInGameUI::get_pending_place_source_object_id();
                (existing != 0).then_some(existing)
            });
        game_client::helpers::TheInGameUI::place_build_available(
            Some(template_name.to_string()),
            source_id,
        );
        log::debug!("BeginStructurePlacement residual: {template_name}");
    }

    /// Pick nearest alive friendly authored dozer for structure placement.
    pub(super) fn find_nearest_friendly_dozer(
        &self,
        player_id: u32,
        location: glam::Vec3,
    ) -> Option<crate::game_logic::ObjectId> {
        // Wave 219: team prefers presentation freeze, then host player boot residual.
        let team = self
            .last_presentation_frame
            .as_ref()
            .map(|f| f.local_team())
            .or_else(|| self.ui_player_team(player_id))
            .unwrap_or(crate::game_logic::Team::USA);
        let frame = self.last_presentation_frame.as_ref()?;
        let cands: Vec<_> = frame
            .objects
            .iter()
            .filter_map(|o| {
                if o.destroyed || o.team != team {
                    return None;
                }
                if !crate::presentation_frame::PresentationFrame::object_has_kind(
                    o,
                    crate::game_logic::KindOf::Dozer,
                ) {
                    return None;
                }
                if !crate::unit_control::UnitControlSystem::presentation_is_selectable(o) {
                    return None;
                }
                Some(
                    crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                        id: o.id,
                        team: o.team,
                        position: o.position,
                        is_alive: true,
                        is_neutral: false,
                        under_construction: o.under_construction,
                        combat_kind: true,
                        effectively_stealthed: false,
                        is_air: false,
                        eject_invulnerable: false,
                    },
                )
            })
            .collect();
        crate::game_logic::host_residual_acquire::pick_nearest_residual_target_xz(
            None,
            (location.x, location.z),
            cands,
            f32::MAX,
            |_| true,
        )
        .map(|(id, _, _)| id)
    }

    pub(super) fn is_wall_structure_template(template_name: &str) -> bool {
        game_client::message_stream::is_line_build_template_name(template_name)
    }

    pub(super) fn host_selection_can_set_rally(&self) -> bool {
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return false;
        };
        let ids = self.ui_selected_ids(self.current_player_id);
        if ids.is_empty() {
            return false;
        }
        ids.iter().all(|id| {
            frame
                .objects
                .iter()
                .find(|o| o.id == *id)
                .map(|o| {
                    !o.destroyed
                        && frame.is_owned_by_local(o)
                        && crate::presentation_frame::PresentationFrame::object_has_kind(
                            o,
                            crate::game_logic::KindOf::AutoRallypoint,
                        )
                })
                .unwrap_or(false)
        })
    }

    /// Presentation-owned object identity for UI/command residual (InGame).
    /// Live GameLogic is boot residual only when no frame is installed.
    #[inline]
    pub(super) fn presentation_ro(
        &self,
        id: crate::game_logic::ObjectId,
    ) -> Option<&crate::presentation_frame::RenderableObject> {
        self.last_presentation_frame
            .as_ref()?
            .objects
            .iter()
            .find(|o| o.id == id)
    }

    /// Wave 580: host cancel-production residual — GameLogic cancel + construction
    /// panel queue head sync (presentation HUD residual).
    pub(super) fn host_cancel_production_and_sync_hud(
        &mut self,
        id: crate::game_logic::ObjectId,
        template_name: String,
    ) -> bool {
        // Wave 580/869/920/931: cancel + HUD residual via object-lifecycle authority.
        // Under presentation freeze, next finalize owns producer scan residual.
        let ok = matches!(
            self.host_game_logic_mut().apply_object_lifecycle_op(
                crate::game_logic::ObjectLifecycleOp::CancelProduction {
                    id,
                    template_name: template_name.clone(),
                },
            ),
            crate::game_logic::ObjectLifecycleResult::Bool(true)
        );
        if !ok {
            return false;
        }
        let panel = &mut self.game_hud.construction_panel;
        if let Some(idx) = panel
            .building_queue
            .iter()
            .rposition(|q| q.item_name == template_name)
        {
            panel.building_queue.remove(idx);
        }
        if self.last_presentation_frame.is_none() {
            self.host_refresh_local_train_producer_residuals();
        }
        true
    }

    /// Cancel the exact production-queue entry represented by a Control Bar
    /// build-queue icon.  Duplicate unit/upgrade names are legal, so the
    /// positional identity must survive the UI → host bridge.
    #[inline]
    pub(super) fn host_cancel_production_at_index(
        &mut self,
        id: crate::game_logic::ObjectId,
        queue_index: usize,
    ) -> bool {
        let ok = matches!(
            self.host_game_logic_mut().apply_object_lifecycle_op(
                crate::game_logic::ObjectLifecycleOp::CancelProductionAtIndex { id, queue_index },
            ),
            crate::game_logic::ObjectLifecycleResult::Bool(true)
        );
        if ok && self.last_presentation_frame.is_none() {
            self.host_refresh_local_train_producer_residuals();
        }
        ok
    }

    fn live_max_select_count() -> i32 {
        #[cfg(feature = "game_client")]
        {
            let v = game_client::helpers::TheInGameUI::get_max_select_count();
            if v != 0 {
                return v;
            }
        }
        game_engine::common::ini::ini_in_game_ui::get_in_game_ui_settings()
            .map(|s| s.max_selection_size)
            .unwrap_or(-1)
    }

    fn cap_selection_ids(
        &mut self,
        mut ids: Vec<crate::game_logic::ObjectId>,
    ) -> Vec<crate::game_logic::ObjectId> {
        let max = Self::live_max_select_count();
        if max > 0 && ids.len() > max as usize {
            ids.truncate(max as usize);
            if !self.displayed_max_selection_warning {
                self.displayed_max_selection_warning = true;
                let msg = format_max_selection_size_message(max);
                self.game_hud.push_info_message(&msg);
                self.ui_manager.game_hud_mut().push_info_message(&msg);
            }
        } else if max > 0 {
            self.displayed_max_selection_warning = false;
        }
        ids
    }

    /// C++ evaluateSoloNexus: clicking a member selects/commands the nexus.
    fn remap_angry_mob_selection_ids(
        &self,
        ids: Vec<crate::game_logic::ObjectId>,
    ) -> Vec<crate::game_logic::ObjectId> {
        use crate::game_logic::host_angry_mob::remap_angry_mob_selection_id;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let remapped = self
                .host_game_logic()
                .host_object(id)
                .map(|o| {
                    remap_angry_mob_selection_id(o.angry_mob_member, o.angry_mob_nexus_id, o.id)
                })
                .unwrap_or(id);
            if !out.contains(&remapped) {
                out.push(remapped);
            }
        }
        out
    }

    /// C++ `ControlBar::onDrawableSelected` / empty `onDrawableDeselected`:
    /// `TheInGameUI->setGUICommand(NULL)`. `selectDrawable` only notifies when
    /// a drawable becomes selected. `setGUICommand` is a playback no-op.
    fn cancel_armed_gui_command_on_selection_change(
        &mut self,
        new_ids: &[crate::game_logic::ObjectId],
    ) {
        if crate::command_system::host_recorder_is_playback() {
            return;
        }
        let newly_selected = new_ids.iter().any(|id| !self.selected_objects.contains(id));
        let deselected_all = new_ids.is_empty() && !self.selected_objects.is_empty();
        if !newly_selected && !deselected_all {
            return;
        }
        if self.pending_map_command.take().is_some() {
            self.clear_radius_cursor_overlays();
        }
        #[cfg(feature = "game_client")]
        {
            game_client::helpers::TheInGameUI::clear_pending_command();
            game_client::helpers::TheInGameUI::clear_pending_special_power();
        }
    }

    /// Wave 579: host selection residual — keep GameLogic selection and engine
    /// `selected_objects` in lockstep.
    #[inline]
    pub(super) fn host_set_selection(
        &mut self,
        player_id: u32,
        ids: Vec<crate::game_logic::ObjectId>,
    ) {
        let ids = self.remap_angry_mob_selection_ids(ids);
        let ids = self.cap_selection_ids(ids);
        self.cancel_armed_gui_command_on_selection_change(&ids);
        // Wave 579/866/913/933: selection residual via session-control authority.
        // Skip authority select_objects when residual already matches.
        let already =
            self.selected_objects == ids && self.host_match_selected_ids.as_ref() == Some(&ids);
        if !already {
            self.host_game_logic_mut().apply_session_control_op(
                crate::game_logic::SessionControlOp::SelectObjects {
                    player_id,
                    ids: ids.clone(),
                },
            );
        }
        self.selected_objects = ids.clone();
        self.host_match_selected_ids = Some(ids.clone());
        // Keep presentation freeze selection residual in lockstep. Otherwise
        // `host_ui_selected_ids_from_residuals` (presentation-first) returns empty
        // until the next dual-tick rebuild, so host select → RMB order fails closed
        // with selected_count>0 but ui_selected_ids empty.
        if let Some(pres) = self.last_presentation_frame.as_mut() {
            let selected_set: std::collections::HashSet<_> = ids.iter().copied().collect();
            for o in &mut pres.objects {
                o.selected = selected_set.contains(&o.id);
            }
            pres.selected = ids;
        }
        let _ = player_id;
    }

    /// C++ `MSG_CREATE_SELECTED_GROUP_NO_SOUND` / `MSG_ADD_TEAM` /
    /// `MSG_REMOVE_FROM_SELECTED_GROUP` — update selection without VoiceSelect.
    pub(super) fn host_set_selection_no_sound(
        &mut self,
        player_id: u32,
        ids: Vec<crate::game_logic::ObjectId>,
    ) {
        let ids = self.remap_angry_mob_selection_ids(ids);
        let ids = self.cap_selection_ids(ids);
        self.cancel_armed_gui_command_on_selection_change(&ids);
        let already =
            self.selected_objects == ids && self.host_match_selected_ids.as_ref() == Some(&ids);
        if !already {
            self.host_game_logic_mut()
                .select_objects_no_sound(player_id, ids.clone());
        }
        self.selected_objects = ids.clone();
        self.host_match_selected_ids = Some(ids.clone());
        if let Some(pres) = self.last_presentation_frame.as_mut() {
            let selected_set: std::collections::HashSet<_> = ids.iter().copied().collect();
            for o in &mut pres.objects {
                o.selected = selected_set.contains(&o.id);
            }
            pres.selected = ids;
        }
        let _ = player_id;
    }

    /// Load the requested map or the default fallback, returning only an
    /// identity that actually reached GameLogic's successful map-load tail.
    /// `None` means neither map could be loaded and callers must not enter a
    /// match state.
    #[inline]
    pub(super) fn host_load_map_or_default(&mut self, map_name: &str) -> Option<String> {
        // Wave 579/871/918/922: load_map_or_fallback residual (single authority boundary).
        // A warm residual alone is not proof that a world exists: a previous
        // failed map attempt may have been interrupted between Loading and its
        // cleanup. Skip only when the authoritative map is also playable and
        // agrees with that identity.
        if self.host_match_map_name.as_deref() == Some(map_name)
            && self.game_logic.isInGame()
            && self.game_logic.get_current_map_name() == map_name
        {
            return Some(map_name.to_string());
        }
        // Clear stale match residuals before map identity changes.
        self.host_clear_match_residuals();
        #[cfg(feature = "game_client")]
        let active_load_screen = if self.loading_overlay_active {
            self.active_load_screen
        } else {
            None
        };
        #[cfg(feature = "game_client")]
        let mut last_load_screen_progress = None::<(f32, String)>;

        let loaded = {
            #[cfg(feature = "game_client")]
            {
                if let Some(kind) = active_load_screen {
                    self.game_logic.load_map_or_fallback_with_progress(
                        map_name,
                        DEFAULT_SKIRMISH_MAP,
                        |progress, phase| {
                            let progress = progress.clamp(0.0, 1.0);
                            Self::pump_cpp_load_screen_progress(kind, progress, phase);
                            last_load_screen_progress = Some((progress, phase.to_string()));
                        },
                    )
                } else {
                    self.game_logic
                        .load_map_or_fallback(map_name, DEFAULT_SKIRMISH_MAP)
                }
            }
            #[cfg(not(feature = "game_client"))]
            {
                self.game_logic
                    .load_map_or_fallback(map_name, DEFAULT_SKIRMISH_MAP)
            }
        };

        #[cfg(feature = "game_client")]
        if loaded.is_some() {
            if let Some(kind) = active_load_screen {
                // C++ `GameLogic::startNewGame` emits LOAD_PROGRESS_END after
                // its map work. The direct host path has no later loading-loop
                // callback, so finish the visible sequence before presentation
                // seeding can move the engine to InGame.
                Self::pump_cpp_load_screen_progress(kind, 1.0, "Map load complete");
                last_load_screen_progress = Some((1.0, "Map load complete".to_string()));
            }
        }

        #[cfg(feature = "game_client")]
        if let Some((progress, phase)) = last_load_screen_progress {
            // The map callback has already updated and drawn the .wnd screen.
            // Only synchronize host-owned status after releasing the mutable
            // GameLogic borrow; calling update_shell_loading_progress here
            // would issue a duplicate final frame pump.
            self.startup_last_reported_progress = progress;
            if !phase.trim().is_empty() {
                self.startup_loading_phase = phase;
            }
        }

        let Some(loaded) = loaded else {
            warn!(
                "Failed to load requested map '{}' and fallback '{}'",
                map_name, DEFAULT_SKIRMISH_MAP
            );
            return None;
        };
        if let Some(shadow) = self.gameworld_shadow.as_mut() {
            shadow.reset_for_world_boundary();
            shadow.sync_from_host(&self.game_logic);
        }
        // The new logical world is now authoritative. Prime exact source W3D
        // topology before a post-load combat tick can accept a shot; failure
        // remains cache-miss/one-barrel fail-closed rather than doing I/O from
        // `Weapon::privateFireWeapon`'s Rust equivalent.
        self.prewarm_host_weapon_barrel_topologies_for_loaded_world();
        // A successful requested or fallback load installed a new terrain
        // payload and a new logical object world.  Rebuild the
        // presentation-owned Arc and discard raw object-id renderer timelines
        // only when that new world is ready; failed attempts leave their
        // active world/cache intact.
        self.host_advance_direct_visual_world_epoch();
        #[cfg(feature = "game_client")]
        self.game_client.invalidate_presentation_drawable_world();
        self.render_pipeline.invalidate_world_visual_state();
        self.invalidate_presentation_terrain_cache();
        if loaded != map_name {
            warn!(
                "Failed to load requested map '{}'; loaded fallback '{}'",
                map_name, loaded
            );
        }
        self.host_match_map_name = Some(loaded.clone());
        self.host_stamp_sim_timing_residuals();
        Some(loaded)
    }

    pub(super) fn host_center_camera_and_request_focus(
        &mut self,
        world_pos: glam::Vec3,
    ) -> glam::Vec3 {
        // Wave 577/868/903: host camera target residual only (no request_camera_focus dual-read).
        // Presentation freeze / Main camera_target own observe path.
        self.cancel_scripted_camera_from_player_look_at();
        let clamped = self.clamp_to_world_bounds(world_pos);
        self.camera_target.x = clamped.x;
        self.camera_target.z = clamped.z;
        // C++ W3DView::lookAt/setPosition rebuilds the active camera rather
        // than waiting for unrelated scroll/shake activity.
        self.apply_camera_orbit_transform();
        if matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.update_mouse_world_position();
            self.sync_context_mouse_cursor();
        }
        clamped
    }

    /// Wave 577: host start-new-game residual with faction team (optional skirmish AI).
    #[inline]
    pub(super) fn host_start_new_game_with_faction(
        &mut self,
        mode: crate::game_logic::GameMode,
        faction_team: crate::game_logic::Team,
        setup_skirmish_ai: bool,
    ) {
        // Wave 577/871/921/933: start_new_game via session-control authority.
        // Clear prior match residuals; stamp mode/team immediately for peels.
        self.host_clear_match_residuals();
        let player_id = self.current_player_id;
        self.host_game_logic_mut().apply_session_control_op(
            crate::game_logic::SessionControlOp::StartNewGameWithFaction {
                mode,
                player_id,
                faction_team,
                setup_skirmish_ai,
            },
        );
        self.host_match_game_mode = Some(mode);
        self.host_match_local_team = Some(faction_team);
        self.host_match_local_player_id = Some(self.current_player_id);
        self.host_stamp_sim_timing_residuals();
    }

    /// Start a Campaign/Challenge session from its exact C++ PlayerTemplate.
    ///
    /// This intentionally has a separate authority payload from the legacy
    /// Team-only helper above: no-template skirmish/runtime callers therefore
    /// cannot inherit a stale selected General.
    #[inline]
    pub(super) fn host_start_new_game_with_player_template(
        &mut self,
        mode: crate::game_logic::GameMode,
        faction_team: crate::game_logic::Team,
        player_template: crate::game_logic::PlayerTemplateIdentity,
    ) -> bool {
        self.host_clear_match_residuals();
        let player_id = self.current_player_id;
        self.host_game_logic_mut().apply_session_control_op(
            crate::game_logic::SessionControlOp::StartNewGameWithPlayerTemplate {
                mode,
                player_id,
                player_template,
            },
        );
        if self
            .game_logic
            .player_template_identity(player_id)
            .is_none()
        {
            // The identity was generation/index validated before this call,
            // but the Common store is still checked at the GameLogic boundary.
            // Do not let a late invalidation proceed into map load as a generic
            // base-faction match.
            log::error!(
                "Rejecting host Campaign/Challenge start: PlayerTemplate binding for player {} did not survive session reset",
                player_id
            );
            return false;
        }
        self.host_match_game_mode = Some(mode);
        self.host_match_local_team = Some(faction_team);
        self.host_match_local_player_id = Some(self.current_player_id);
        self.host_stamp_sim_timing_residuals();
        true
    }

    pub(super) fn host_process_commands_with_command_sound(&mut self) {
        // Wave 576/870/914/915/918/932: process + Command SFX via command-pipeline authority.
        // Empty queue skips process dual-write and Command SFX (no has_pending dual-read).
        if !self
            .game_logic
            .apply_command_pipeline_op(crate::game_logic::CommandPipelineOp::ProcessIfNeeded)
        {
            return;
        }
        self.play_sound_effect(SoundType::Command);
        // Skip mid-command stamp when presentation freeze owns clocks.
        if self.last_presentation_frame.is_none() {
            self.host_stamp_sim_timing_residuals();
        }
    }

    /// Wave 584: host queue-command residual (no immediate process flush).
    #[inline]
    pub(super) fn host_queue_command(&mut self, command: crate::command_system::GameCommand) {
        // Wave 584/872/874/916/932: host queue residual via command-pipeline authority.
        // Queue-only path does not stamp sim timing — process/tick residuals own clocks.
        let _ = self
            .game_logic
            .apply_command_pipeline_op(crate::game_logic::CommandPipelineOp::Queue { command });
    }

    /// Wave 576: queue a GameCommand then flush with Command SFX.
    #[inline]
    pub(super) fn host_queue_and_process_command(
        &mut self,
        command: crate::command_system::GameCommand,
    ) {
        // Wave 576/874: queue + process + Command SFX residual via host helpers.
        self.host_queue_command(command);
        self.host_process_commands_with_command_sound();
    }

    /// Wave 576/578: queue + process without Command SFX (upgrade/honesty/UI residual paths).
    /// Wave 578: force_attack/construct/science residual peels use this helper.
    #[inline]
    pub(super) fn host_queue_and_process_command_silent(
        &mut self,
        command: crate::command_system::GameCommand,
    ) {
        // Wave 576/578/871/914/918/922/932: silent queue+process via command-pipeline authority.
        // Skip mid-command stamp when presentation freeze owns clocks.
        let processed = self.host_game_logic_mut().apply_command_pipeline_op(
            crate::game_logic::CommandPipelineOp::QueueAndProcess { command },
        );
        if processed && self.last_presentation_frame.is_none() {
            self.host_stamp_sim_timing_residuals();
        }
    }

    pub(super) fn host_set_paused(&mut self, paused: bool) {
        // Wave 575/601/867/892/913/933: pause residual via session-control authority.
        // Engine.game_paused and GameLogic.is_paused must stay one flag like C++
        // GameEngine.cpp:749. The residual skip only avoids rewriting the host
        // latch; the session-control op must still land or a leftover quit/load
        // pause pins logic_frame at 0 while status reports paused=false.
        if self.game_paused != paused {
            self.game_paused = paused;
        }
        self.host_game_logic_mut()
            .apply_session_control_op(crate::game_logic::SessionControlOp::SetPaused { paused });

        // Compose freeze residual without dual-read when presentation freeze owns
        // script time (InGame). Boot path still probes live is_time_frozen once.
        let script_frozen = if let Some(pres) = self.last_presentation_frame.as_ref() {
            pres.time_frozen_for_simulation
        } else {
            // Wave 902: fail-closed boot residual (no is_time_frozen dual-read).
            false
        };
        self.host_match_time_frozen = Some(script_frozen || paused);
    }

    pub(super) fn boot_local_player_id_from_host(&self) -> u32 {
        // Wave 574/892/897: prefer stamped host_match_local_player_id before host residual.
        if let Some(id) = self.host_match_local_player_id {
            return id;
        }
        // Wave 897: host current_player_id residual (no player_exists/min_player dual-read).
        self.current_player_id
    }

    /// Local/human player id for UI command issue. Prefers presentation freeze.
    /// Wave 574: boot path via `boot_local_player_id_from_host`.
    /// Wave 607: via `host_local_player_id_for_ui`.
    pub(super) fn local_player_id_for_ui(&self) -> u32 {
        // Wave 607: thin wrapper — UI residual via host helper.
        self.host_local_player_id_for_ui()
    }

    /// Local/human player id for UI command issue. Prefers presentation freeze.
    /// Wave 574: boot path via `boot_local_player_id_from_host`.
    pub(super) fn host_local_player_id_for_ui(&self) -> u32 {
        // Wave 607: host UI residual helper.
        // Wave 240/555: presentation freeze owns local player residual when installed.
        if self.last_presentation_frame.is_some() {
            return self
                .presentation_or_boot_local_player_id()
                .unwrap_or(self.current_player_id);
        }
        // Wave 574: boot residual via shared host probe helper.
        self.boot_local_player_id_from_host()
    }

    /// Local team for UI. Prefers presentation freeze.
    /// Wave 607: via `host_local_team_for_ui`.
    pub(super) fn local_team_for_ui(&self) -> crate::game_logic::Team {
        // Wave 607: thin wrapper — UI residual via host helper.
        self.host_local_team_for_ui()
    }

    /// Local team for UI. Prefers presentation freeze.
    pub(super) fn host_local_team_for_ui(&self) -> crate::game_logic::Team {
        // Wave 607: host UI residual helper.
        // Wave 240/555: via presentation_or_boot_local_team helper.
        self.presentation_or_boot_local_team()
    }

    /// Wave 573: boot residual player roster probe (no presentation freeze).
    /// Shared by `ui_player_info` and `presentation_or_boot_diplomacy_players`.
    pub(super) fn boot_player_info_from_host(
        &self,
        player_id: u32,
    ) -> Option<crate::presentation_frame::PresentationPlayerInfo> {
        // Wave 573/897: prefer stamped diplomacy residual / presentation freeze.
        if let Some(players) = self.host_match_diplomacy_players.as_ref() {
            return players.iter().find(|p| p.id == player_id).cloned();
        }
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return pres.players.iter().find(|p| p.id == player_id).cloned();
        }
        // Wave 897: fail-closed boot default (no dual-read).
        let _ = player_id;
        None
    }

    pub(super) fn ui_player_info(
        &self,
        player_id: u32,
    ) -> Option<crate::presentation_frame::PresentationPlayerInfo> {
        // Wave 607: thin wrapper — UI residual via host helper.
        self.host_ui_player_info(player_id)
    }

    /// Wave 234/549: player roster probe prefers presentation freeze.
    /// When freeze is installed, missing player_info fails closed (no host
    /// player_* dual-read mid-frame). Boot residual without freeze unchanged.
    /// Wave 573: boot path via `boot_player_info_from_host`.
    pub(super) fn host_ui_player_info(
        &self,
        player_id: u32,
    ) -> Option<crate::presentation_frame::PresentationPlayerInfo> {
        // Wave 607/846: host UI residual helper.
        if let Some(frame) = self.last_presentation_frame.as_ref() {
            // Wave 549: presentation freeze owns player roster residual — even if miss.
            return frame.player_info(player_id).cloned();
        }
        if let Some(players) = self.host_match_diplomacy_players.as_ref() {
            return players.iter().find(|p| p.id == player_id).cloned();
        }
        // Wave 573: boot residual via shared host probe helper.
        self.boot_player_info_from_host(player_id)
    }

    #[inline]
    /// Wave 607: via `host_ui_player_team`.
    pub(super) fn ui_player_team(&self, player_id: u32) -> Option<crate::game_logic::Team> {
        // Wave 607: thin wrapper — UI residual via host helper.
        self.host_ui_player_team(player_id)
    }

    #[inline]
    pub(super) fn host_ui_player_team(&self, player_id: u32) -> Option<crate::game_logic::Team> {
        // Wave 607: host UI residual helper.
        self.ui_player_info(player_id).map(|p| p.team)
    }

    #[inline]
    /// Wave 607: via `host_ui_player_name`.
    pub(super) fn ui_player_name(&self, player_id: u32) -> Option<String> {
        // Wave 607: thin wrapper — UI residual via host helper.
        self.host_ui_player_name(player_id)
    }

    #[inline]
    pub(super) fn host_ui_player_name(&self, player_id: u32) -> Option<String> {
        // Wave 607: host UI residual helper.
        self.ui_player_info(player_id).map(|p| p.name)
    }

    #[inline]
    /// Wave 575: local team name via presentation_or_boot_local_team (freeze prefer).
    /// Wave 607: via `host_ui_local_player_team_name`.
    pub(super) fn ui_local_player_team_name(&self) -> Option<String> {
        // Wave 607: thin wrapper — UI residual via host helper.
        self.host_ui_local_player_team_name()
    }

    #[inline]
    /// Wave 575: local team name via presentation_or_boot_local_team (freeze prefer).
    pub(super) fn host_ui_local_player_team_name(&self) -> Option<String> {
        // Wave 607: host UI residual helper.
        // Wave 575: prefer presentation-or-boot local team residual.
        Some(
            self.presentation_or_boot_local_team()
                .get_name()
                .to_string(),
        )
    }

    /// Wave 234: selection seed prefers engine/presentation over live player dual-read.
    /// Wave 252: script default camera residuals via presentation freeze.
    /// Wave 607: via `host_ui_script_default_camera_max_height`.
    pub(super) fn ui_script_default_camera_max_height(&self) -> f32 {
        // Wave 607: thin wrapper — UI residual via host helper.
        self.host_ui_script_default_camera_max_height()
    }

    /// Wave 234: selection seed prefers engine/presentation over live player dual-read.
    /// Wave 252: script default camera residuals via presentation freeze.
    pub(super) fn host_ui_script_default_camera_max_height(&self) -> f32 {
        // Wave 607/858: host UI residual helper.
        // Wave 252: presentation freeze first.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return pres.script_default_camera_max_height;
        }
        if let Some(v) = self.host_match_script_camera_max_height {
            return v;
        }
        // Wave 898: fail-closed boot default (C++ residual 1.0).
        1.0
    }

    /// Wave 609: via `host_ui_script_default_camera_pitch`.
    pub(super) fn ui_script_default_camera_pitch(&self) -> f32 {
        // Wave 609: thin wrapper — UI/presentation residual via host helper.
        self.host_ui_script_default_camera_pitch()
    }

    pub(super) fn host_ui_script_default_camera_pitch(&self) -> f32 {
        // Wave 609/858: host UI/presentation residual helper.
        // Wave 252: presentation freeze first.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return pres.script_default_camera_pitch;
        }
        if let Some(v) = self.host_match_script_camera_pitch {
            return v;
        }
        // Wave 898: fail-closed boot default (C++ residual 1.0).
        1.0
    }

    /// Wave 609: via `host_ui_selection_seed_id`.
    pub(super) fn ui_selection_seed_id(&self) -> Option<crate::game_logic::ObjectId> {
        // Wave 609: thin wrapper — UI/presentation residual via host helper.
        self.host_ui_selection_seed_id()
    }

    pub(super) fn host_ui_selection_seed_id(&self) -> Option<crate::game_logic::ObjectId> {
        // Wave 609/850: host UI/presentation residual helper.
        // Wave 215/544: prefer engine selection residual, then presentation freeze.
        // When a presentation freeze is installed, empty selection seed fails closed
        // (no host player_selected_objects dual-read mid-frame).
        if let Some(id) = self.selected_objects.first().copied() {
            return Some(id);
        }
        if let Some(frame) = self.last_presentation_frame.as_ref() {
            if let Some(id) = frame.selected.first().copied() {
                return Some(id);
            }
            if let Some(id) = frame.selection_ids_for_consumers().first().copied() {
                return Some(id);
            }
            // Wave 544: presentation freeze owns selection seed residual — even if empty.
            return None;
        }
        // Wave 850/905: host-stamped selection residual before fail-closed boot.
        if let Some(ids) = self.host_match_selected_ids.as_ref() {
            return ids.first().copied();
        }
        // Wave 905: fail-closed boot default (no player_selected_objects dual-read).
        None
    }

    /// Wave 234: local science purchase points prefer presentation freeze.
    /// Wave 610: via `host_ui_local_science_purchase_points`.
    pub(super) fn ui_local_science_purchase_points(&self) -> i32 {
        // Wave 610: thin wrapper — residual via host helper.
        self.host_ui_local_science_purchase_points()
    }

    /// Wave 234: local science purchase points prefer presentation freeze.
    pub(super) fn host_ui_local_science_purchase_points(&self) -> i32 {
        // Wave 610/868: host residual helper.
        // Presentation freeze first, then host-stamped residual, then boot probe.
        if let Some(frame) = self.last_presentation_frame.as_ref() {
            return frame.local_science_purchase_points();
        }
        if let Some(v) = self.host_match_local_science_purchase_points {
            return v;
        }
        // Wave 905: fail-closed boot default (no player_science dual-read).
        0
    }

    /// Wave 238: local economy prefers presentation freeze.
    /// Wave 609: via `host_ui_local_economy`.
    pub(super) fn ui_local_economy(
        &self,
    ) -> (
        i32, /*money*/
        i32, /*power*/
        i32, /*max_power*/
    ) {
        // Wave 609: thin wrapper — UI/presentation residual via host helper.
        self.host_ui_local_economy()
    }

    /// Wave 238: local economy prefers presentation freeze.
    pub(super) fn host_ui_local_economy(
        &self,
    ) -> (
        i32, /*money*/
        i32, /*power*/
        i32, /*max_power*/
    ) {
        // Wave 609: host UI/presentation residual helper.
        if let Some(frame) = self.last_presentation_frame.as_ref() {
            let money = frame.local_supplies as i32;
            let power = frame.local_power;
            let max_power = frame.local_power_produced.max(0);
            return (money, power, max_power);
        }
        // Wave 905: fail-closed boot default (no player_economy dual-read).
        (0, 0, 0)
    }

    #[inline]
    pub(super) fn ui_object_alive(&self, id: crate::game_logic::ObjectId) -> bool {
        // Presentation-only identity for InGame UI residual.
        self.presentation_ro(id)
            .is_some_and(|o| !o.destroyed && o.health_current > 0.0)
    }

    #[inline]
    pub(super) fn ui_object_is_dozer(&self, id: crate::game_logic::ObjectId) -> bool {
        // Presentation-only identity for InGame UI residual.
        let Some(o) = self.presentation_ro(id) else {
            return false;
        };
        if o.destroyed || o.health_current <= 0.0 {
            return false;
        }
        crate::presentation_frame::PresentationFrame::object_has_kind(
            o,
            crate::game_logic::KindOf::Dozer,
        )
    }

    #[inline]
    pub(super) fn ui_object_can_produce(&self, id: crate::game_logic::ObjectId) -> bool {
        // Wave 215: presentation-only (no live GameLogic dual-read residual).
        self.presentation_ro(id).is_some_and(|o| {
            o.can_produce && !o.destroyed && !o.under_construction && o.health_current > 0.0
        })
    }

    #[inline]
    pub(super) fn ui_object_under_construction(&self, id: crate::game_logic::ObjectId) -> bool {
        // Wave 215: presentation-only (no live GameLogic dual-read residual).
        self.presentation_ro(id)
            .is_some_and(|o| o.under_construction && !o.destroyed && o.health_current > 0.0)
    }

    #[inline]
    pub(super) fn ui_production_queue_head(
        &self,
        id: crate::game_logic::ObjectId,
    ) -> Option<String> {
        // Presentation-only identity for InGame UI residual.
        self.presentation_ro(id)
            .and_then(|o| o.production_queue.first().map(|p| p.template_name.clone()))
    }

    #[inline]
    pub(super) fn ui_special_power_ready(&self, id: crate::game_logic::ObjectId) -> bool {
        // Wave 215: presentation-only (no live GameLogic dual-read residual).
        self.presentation_ro(id).is_some_and(|o| {
            o.special_power_ready && !o.destroyed && o.health_current > 0.0 && !o.under_construction
        })
    }

    /// Presentation special-power type residual when ready.
    #[inline]
    pub(super) fn ui_special_power_type_if_ready(
        &self,
        id: crate::game_logic::ObjectId,
    ) -> Option<crate::command_system::SpecialPowerType> {
        // Presentation-only identity for InGame UI residual.
        let o = self.presentation_ro(id)?;
        if o.destroyed || o.health_current <= 0.0 {
            return None;
        }
        o.special_power_ready_template_name
            .as_deref()
            .and_then(crate::command_system::special_power_type_from_template_name)
    }

    /// Wave 610: via `host_ui_selected_ids`.
    pub(super) fn ui_selected_ids(&self, player_id: u32) -> Vec<crate::game_logic::ObjectId> {
        // Wave 610: thin wrapper — residual via host helper.
        self.host_ui_selected_ids(player_id)
    }

    pub(super) fn host_ui_selected_ids(&self, player_id: u32) -> Vec<crate::game_logic::ObjectId> {
        // Wave 610: host residual helper.
        // Wave 215: presentation freeze owns InGame selection residual (fail-closed)
        // even if empty). No GameLogic get_player / player_selected_objects dual-read.
        let _ = player_id;
        crate::game_logic::host_ui_selected_ids_from_residuals(
            self.last_presentation_frame.as_ref(),
            &self.selected_objects,
            self.host_match_selected_ids.as_deref(),
        )
    }

    pub(super) fn place_structure_from_ui(&mut self, template_name: &str, location: glam::Vec3) {
        use crate::game_logic::host_production_buildable_command_residual::{
            LBC_NO_CLEAR_PATH, LBC_NOT_FLAT_ENOUGH, LBC_OBJECTS_IN_THE_WAY, LBC_OK,
            LBC_RESTRICTED_TERRAIN, LBC_SHROUD, LBC_TOO_CLOSE_TO_SUPPLIES,
        };

        let template = resolve_ui_structure_template_name(template_name);
        if template.is_empty() || !location.x.is_finite() || !location.z.is_finite() {
            return;
        }

        // Prefer presentation local player/team freeze; selected from engine selection residual.
        let player_id = self.local_player_id_for_ui();
        let team = self.local_team_for_ui();

        // Wave 219: selection via presentation-first ui_selected_ids.
        let mut selected = self.ui_selected_ids(player_id);
        let is_dozer = |id: crate::game_logic::ObjectId| self.ui_object_is_dozer(id);
        let dozers: Vec<_> = selected
            .iter()
            .copied()
            .filter(|&id| is_dozer(id))
            .collect();
        if !dozers.is_empty() {
            selected = dozers;
        }
        // C++ residual: if no builder in selection, auto-pick nearest friendly dozer/worker.
        if selected.is_empty() || !selected.iter().any(|&id| is_dozer(id)) {
            if let Some(auto) = self.find_nearest_friendly_dozer(player_id, location) {
                selected = vec![auto];
                self.host_set_selection(player_id, selected.clone());
            }
        }
        if selected.is_empty() {
            log::debug!("PlaceStructureAt ignored — no dozer/worker selection");
            // Keep placement armed so player can select a dozer and retry.
            self.pending_structure_placement = Some(template_name.to_string());
            self.game_hud
                .construction_panel
                .arm_structure_placement(template_name.to_string());
            self.ui_manager
                .game_hud_mut()
                .construction_panel
                .arm_structure_placement(template_name.to_string());
            return;
        }

        let builder_id = selected.first().copied();
        if let Some(id) = builder_id {
            if let Some(pending) = game_client::helpers::TheInGameUI::get_pending_special_power() {
                if pending.source_object_id == id.0 {
                    self.pending_structure_placement = None;
                    self.game_hud.construction_panel.clear_structure_placement();
                    self.ui_manager
                        .game_hud_mut()
                        .construction_panel
                        .clear_structure_placement();
                    let placement_angle = game_client::helpers::TheInGameUI::get_placement_angle();
                    self.host_queue_and_process_command_silent(
                        crate::command_system::GameCommand {
                            command_type: crate::command_system::CommandType::DoSpecialPower {
                                power_type: crate::command_system::SpecialPowerType::SneakAttack,
                                target: crate::command_system::PowerTarget::LocationFacing {
                                    pos: location,
                                    angle: placement_angle,
                                },
                            },
                            player_id,
                            command_id: 0,
                            timestamp: std::time::SystemTime::now(),
                            selected_units: selected.clone(),
                            modifier_keys: crate::command_system::ModifierKeys::default(),
                        },
                    );
                    game_client::helpers::TheInGameUI::place_build_available(None, None);
                    game_client::helpers::TheInGameUI::clear_pending_special_power();
                    return;
                }
            }
        }

        if let Some(id) = builder_id {
            if let Some(builder) = gamelogic::helpers::TheGameLogic::find_object_by_id(id.0) {
                if let Ok(guard) = builder.read() {
                    if let Some(tmpl) =
                        gamelogic::helpers::TheThingFactory::find_template(&template)
                    {
                        let pending =
                            game_client::helpers::TheInGameUI::get_pending_special_power();
                        let cmt = game_client::message_stream::can_make_unit_for_place(
                            &guard,
                            tmpl.as_ref(),
                            pending.as_ref(),
                        );
                        if cmt != game_engine::common::system::build_assistant::CanMakeType::Ok {
                            game_client::message_stream::play_can_make_failure(cmt);
                            if matches!(
                                cmt,
                                game_engine::common::system::build_assistant::CanMakeType::NoMoney
                                    | game_engine::common::system::build_assistant::CanMakeType::QueueFull
                                    | game_engine::common::system::build_assistant::CanMakeType::ParkingPlacesFull
                                    | game_engine::common::system::build_assistant::CanMakeType::MaxedOutForPlayer
                            ) {
                                return;
                            }
                            game_client::helpers::TheInGameUI::place_build_available(None, None);
                            self.pending_structure_placement = None;
                            return;
                        }
                    }
                }
            }
        }

        let lbc = self.host_legal_build_code_at_for_builder(team, location, &template, builder_id);
        if lbc != LBC_OK {
            self.pending_structure_placement = Some(template_name.to_string());
            self.game_hud
                .construction_panel
                .arm_structure_placement(template_name.to_string());
            self.ui_manager
                .game_hud_mut()
                .construction_panel
                .arm_structure_placement(template_name.to_string());
            // C++ InGameUI::displayCantBuildMessage — leftover map_cant_build_message.
            let cpp_key = match lbc {
                LBC_RESTRICTED_TERRAIN => "GUI:CantBuildRestrictedTerrain",
                LBC_NOT_FLAT_ENOUGH => "GUI:CantBuildNotFlatEnough",
                LBC_OBJECTS_IN_THE_WAY => "GUI:CantBuildObjectsInTheWay",
                LBC_NO_CLEAR_PATH => "GUI:CantBuildNoClearPath",
                LBC_SHROUD => "GUI:CantBuildShroud",
                LBC_TOO_CLOSE_TO_SUPPLIES => "GUI:CantBuildTooCloseToSupplies",
                _ => "GUI:CantBuildThere",
            };
            let msg = game_client::helpers::map_cant_build_message(cpp_key);
            self.game_hud.push_info_message(&msg);
            self.ui_manager.game_hud_mut().push_info_message(&msg);
            if let Some(id) = builder_id {
                play_host_illegal_place_feedback(self, id);
            }

            return;
        }

        self.pending_structure_placement = None;
        // C++ InGameUI.cpp:2981: leaving place-build with a source dozer
        // protects the next alternate-mouse blank LMB from deselecting.
        self.host_set_prevent_left_click_deselection(true);

        self.game_hud.construction_panel.clear_structure_placement();
        self.ui_manager
            .game_hud_mut()
            .construction_panel
            .clear_structure_placement();
        self.play_sound_effect(SoundType::Command);

        self.host_queue_and_process_command_silent(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::DozerConstruct {
                template_name: template,
                location,
                orientation: self
                    .game_hud
                    .construction_panel
                    .placement_preview()
                    .facing_radians,
            },
            player_id,
            command_id: 0,
            timestamp: std::time::SystemTime::now(),
            selected_units: selected,
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
    }

    pub(super) fn place_wall_line_from_ui(
        &mut self,
        template_name: &str,
        start: glam::Vec3,
        end: glam::Vec3,
    ) {
        let template = template_name.to_string();
        let player_id = self.current_player_id;
        // Wave 219: selection via presentation-first ui_selected_ids.
        let selected = self.ui_selected_ids(player_id);
        // Prefer dozers/workers in selection residual.
        let builders: Vec<_> = selected
            .iter()
            .copied()
            .filter(|&id| self.ui_object_is_dozer(id))
            .collect();
        let units = if builders.is_empty() {
            selected
        } else {
            builders
        };
        if units.is_empty() {
            return;
        }

        // Keep placement armed for chained wall segments residual.
        self.play_sound_effect(SoundType::Command);
        self.host_queue_and_process_command_silent(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::DozerConstructLine {
                template_name: template,
                start,
                end,
            },
            player_id,
            command_id: 0,
            timestamp: std::time::SystemTime::now(),
            selected_units: units,
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
    }

    /// Cancel production queue head on selected producers residual (Delete key).
    pub(super) fn cancel_selected_production_queue_head(&mut self) -> bool {
        let player_id = self.current_player_id;
        // Wave 219: selection via presentation-first ui_selected_ids.
        let selected = self.ui_selected_ids(player_id);
        if selected.is_empty() {
            return false;
        }
        let mut any = false;
        for id in selected {
            let head_name = self.ui_production_queue_head(id);
            let Some(template_name) = head_name else {
                continue;
            };
            // Wave 580: host cancel + HUD residual via helper.
            if self.host_cancel_production_and_sync_hud(id, template_name) {
                any = true;
            }
        }
        if any {
            self.play_sound_effect(SoundType::Command);
        }
        any
    }

    /// Cancel entire production queue on selected producers residual (Ctrl+Delete).
    pub(super) fn cancel_all_selected_production(&mut self) -> bool {
        let player_id = self.current_player_id;
        // Wave 219: selection via presentation-first ui_selected_ids.
        let selected = self.ui_selected_ids(player_id);
        if selected.is_empty() {
            return false;
        }
        let mut any = false;
        for id in selected {
            // Drain queue head repeatedly residual.
            loop {
                let head_name = self.ui_production_queue_head(id);
                let Some(template_name) = head_name else {
                    break;
                };
                // Wave 580: host cancel + HUD residual via helper.
                if !self.host_cancel_production_and_sync_hud(id, template_name) {
                    break;
                }
                any = true;
            }
        }
        if any {
            self.play_sound_effect(SoundType::Command);
        }
        any
    }

    pub(super) fn cancel_unit_production_from_ui(
        &mut self,
        template_name: &str,
        production_id: u32,
        queue_index: usize,
    ) {
        let _ = template_name;
        let player_id = self.current_player_id;
        let selected = self.ui_selected_ids(player_id);
        if selected.is_empty() {
            return;
        }
        let producers: Vec<_> = selected
            .iter()
            .copied()
            .filter(|&id| self.ui_object_can_produce(id))
            .collect();
        let targets = if producers.is_empty() {
            selected
        } else {
            producers
        };
        // C++ ControlBarCommandProcessing.cpp:466-479 — cancel the clicked
        // slot productionID. Host ProductionItem aliases production_id as index.
        let cancel_index = if production_id != 0 {
            production_id as usize
        } else {
            queue_index
        };
        let mut any = false;
        // C++ ControlBarCommandProcessing.cpp:469-479 — one producer, one slot.
        if let Some(&id) = targets.first() {
            if self.host_cancel_production_at_index(id, cancel_index) {
                any = true;
            }
        }
        if any {
            self.play_sound_effect(SoundType::Command);
            let panel = &mut self.game_hud.construction_panel;
            if let Some(idx) = panel
                .building_queue
                .iter()
                .position(|q| q.production_id == production_id && q.queue_index == queue_index)
            {
                panel.building_queue.remove(idx);
            }
        }
    }

    pub(super) fn cancel_upgrade_production_from_ui(
        &mut self,
        upgrade_name: &str,
        production_id: u32,
        queue_index: usize,
    ) {
        let player_id = self.current_player_id;
        let selected = self.ui_selected_ids(player_id);
        if selected.is_empty() {
            return;
        }
        let producers: Vec<_> = selected
            .iter()
            .copied()
            .filter(|&id| self.ui_object_can_produce(id))
            .collect();
        let targets = if producers.is_empty() {
            selected
        } else {
            producers
        };
        // C++ ControlBarCommandProcessing.cpp:592-601 — MSG_CANCEL_UPGRADE
        // by name on the single UI producer. Do not also cancel_at_index:
        // execute_cancel_upgrade already refunds and strips the queue entry.
        if let Some(&id) = targets.first() {
            self.host_queue_and_process_command_silent(crate::command_system::GameCommand {
                command_type: crate::command_system::CommandType::CancelUpgrade {
                    upgrade_name: upgrade_name.to_string(),
                },
                player_id,
                command_id: 0,
                timestamp: std::time::SystemTime::now(),
                selected_units: vec![id],
                modifier_keys: crate::command_system::ModifierKeys::default(),
            });
        }
        let panel = &mut self.game_hud.construction_panel;
        if let Some(idx) = panel.building_queue.iter().position(|q| {
            q.is_upgrade && q.production_id == production_id && q.queue_index == queue_index
        }) {
            panel.building_queue.remove(idx);
        }
    }

    pub(super) fn queue_unit_production_from_ui(&mut self, template_name: &str, quantity: u32) {
        if template_name.trim().is_empty() || quantity == 0 {
            return;
        }
        let player_id = self.local_player_id_for_ui();
        let selected = self.ui_selected_ids(player_id);
        if selected.is_empty() {
            log::debug!(
                "QueueUnitProduction ignored — no selection for '{}'",
                template_name
            );
            return;
        }
        // Prefer constructed producers in selection residual (presentation-first).
        let producers: Vec<_> = selected
            .iter()
            .copied()
            .filter(|&id| self.ui_object_can_produce(id))
            .collect();
        let units = if producers.is_empty() {
            selected
        } else {
            producers
        };
        self.host_queue_and_process_command_silent(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::QueueUnitCreate {
                template_name: template_name.to_string(),
                quantity,
            },
            player_id,
            command_id: 0,
            timestamp: std::time::SystemTime::now(),
            selected_units: units,
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
    }

    /// C++ ControlBar named command button residual (Upgrade/Cancel/Stop/…).

    pub(super) fn arm_pending_unit_ability(&mut self, ability: PendingUnitAbility) {
        self.pending_map_command = Some(PendingMapCommand::UnitAbility(ability));
        self.pending_structure_placement = None;
        self.arm_radius_cursor_for_pending("OFFENSIVE_SPECIALPOWER");
    }

    pub(super) fn issue_named_command_from_ui(&mut self, command_name: &str) {
        let Some(command_type) = crate::command_system::command_type_from_button_name(command_name)
        else {
            log::debug!("IssueCommand unmapped: {command_name}");
            return;
        };

        // C++ ControlBarCommandProcessing.cpp:183-189 — every command-button
        // activation plays UnitSpecificSound with the local player index.
        play_named_command_button_unit_specific_sound(command_name, self.local_player_id_for_ui());

        // C++ ControlBar: AttackMove/Guard/SetRally wait for map click residual.
        match command_type {
            crate::command_system::CommandType::AttackMoveTo { .. } => {
                self.pending_map_command = Some(PendingMapCommand::AttackMove);
                self.pending_structure_placement = None;
                self.arm_radius_cursor_for_pending("ATTACK_CONTINUE_AREA");
                return;
            }
            crate::command_system::CommandType::Guard { mode, .. } => {
                self.pending_map_command = Some(PendingMapCommand::Guard(mode));
                self.pending_structure_placement = None;
                self.arm_radius_cursor_for_pending("GUARD_AREA");
                return;
            }
            crate::command_system::CommandType::SetRallyPoint { .. } => {
                self.pending_map_command = Some(PendingMapCommand::SetRallyPoint);
                self.pending_structure_placement = None;
                self.arm_radius_cursor_for_pending("FRIENDLY_SPECIALPOWER");
                return;
            }
            crate::command_system::CommandType::CombatDrop { .. } => {
                self.pending_map_command = Some(PendingMapCommand::CombatDrop(
                    PendingCombatDropCommand::position_only(),
                ));
                self.pending_structure_placement = None;
                self.arm_radius_cursor_for_pending("COMBATDROP");
                return;
            }
            crate::command_system::CommandType::DoSpecialPower { ref power_type, .. }
                if crate::command_system::leftover_special_power_is_no_target(power_type) =>
            {
                // Leftover ActionManager::can_do_special_power / C++ CommandXlat
                // no-option MSG_DO_SPECIAL_POWER. CIA, CommunicationsDownload,
                // DetonateDirtyNuke, RemoteCharges detonate, BattlePlan*, Baikonur.
            }
            crate::command_system::CommandType::DoSpecialPower { power_type, .. } => {
                // Resolve SW type from selected ready structure residual.
                // Named buttons (SpySatellite, ParticleCannon, …) prefer their type.
                let player_id = self.current_player_id;
                // Wave 219: selection via presentation-first ui_selected_ids.
                let selected = self.ui_selected_ids(player_id);
                let requested = power_type.clone();
                let mut resolved = None;
                // Pass 1: honor named button power when ready on selection.
                // Prefer presentation special_power_ready residual; live host is fallback.
                for id in &selected {
                    if self.ui_special_power_ready(*id) {
                        if let Some(p) = self.ui_special_power_type_if_ready(*id) {
                            if let Some(parsed) =
                                exact_parsed_structure_power_for_button(&requested, p)
                            {
                                // Keep the exact parsed module identity.  The shared
                                // UI button family is presentation-only; execution
                                // must charge and fire the selected retail variant.
                                resolved = Some(parsed);
                                break;
                            }
                        }
                    }
                    // Live host special-power ready is boot residual only (no presentation UI residual).
                    // Wave 584: boot special-power ready residual via helper.
                    if self.last_presentation_frame.is_none()
                        && self.host_is_special_power_ready_for(*id, &requested)
                    {
                        resolved = Some(requested.clone());
                        break;
                    }
                }
                // Pass 2: any ready superweapon structure (generic Command_SpecialPower / V).
                if resolved.is_none() {
                    for id in &selected {
                        if let Some(p) = self.ui_special_power_type_if_ready(*id) {
                            resolved = Some(p);
                            break;
                        }
                        // Presentation special_power residual is authoritative for InGame UI.
                    }
                }
                let Some(power) = resolved else {
                    return;
                };
                if crate::command_system::leftover_special_power_is_no_target(&power) {
                    let player_id = self.local_player_id_for_ui();
                    let selected = self.ui_selected_ids(player_id);
                    if selected.is_empty() {
                        return;
                    }
                    self.host_queue_and_process_command_silent(
                        crate::command_system::GameCommand {
                            command_type: crate::command_system::CommandType::DoSpecialPower {
                                power_type: power,
                                target: crate::command_system::PowerTarget::None,
                            },
                            player_id,
                            command_id: 0,
                            timestamp: std::time::SystemTime::now(),
                            selected_units: selected,
                            modifier_keys: crate::command_system::ModifierKeys::default(),
                        },
                    );
                    return;
                }
                // Map before move into pending.  The same table also serves
                // native ControlBar special-power requests, so faction module
                // variants retain their authored radius cursor.
                let cursor = Self::radius_cursor_type_for_special_power(&power);
                self.pending_map_command = Some(PendingMapCommand::SpecialPower(power));
                self.pending_structure_placement = None;
                self.arm_radius_cursor_for_pending(cursor);
                return;
            }
            crate::command_system::CommandType::PlaceBeacon { .. } => {
                // C++ MSG_META_PLACE_BEACON: MP and not replay; refuse at MaxBeacons.
                if !self.host_command_xlat_multiplayer_meta() {
                    return;
                }
                let player_id = self.local_player_id_for_ui();
                if !crate::command_executor::host_local_player_can_place_beacon(
                    &self.game_logic,
                    player_id,
                ) {
                    return;
                }
                self.pending_map_command = Some(PendingMapCommand::PlaceBeacon);
                self.pending_structure_placement = None;
                self.arm_radius_cursor_for_pending("RADAR");
                return;
            }
            // Unit special-ability buttons: arm target click residual.
            crate::command_system::CommandType::Hijack { .. } => {
                self.arm_pending_unit_ability(PendingUnitAbility::Hijack);
                return;
            }
            crate::command_system::CommandType::Sabotage { .. } => {
                self.arm_pending_unit_ability(PendingUnitAbility::Sabotage);
                return;
            }
            crate::command_system::CommandType::CaptureBuilding { .. } => {
                self.arm_pending_unit_ability(PendingUnitAbility::CaptureBuilding);
                return;
            }
            crate::command_system::CommandType::SnipeVehicle { .. } => {
                self.arm_pending_unit_ability(PendingUnitAbility::SnipeVehicle);
                return;
            }
            crate::command_system::CommandType::PlantTimedDemoCharge { .. } => {
                self.arm_pending_unit_ability(PendingUnitAbility::PlantTimedDemoCharge);
                return;
            }
            crate::command_system::CommandType::PlantRemoteDemoCharge { .. } => {
                self.arm_pending_unit_ability(PendingUnitAbility::PlantRemoteDemoCharge);
                return;
            }
            crate::command_system::CommandType::StealCashHack { .. } => {
                self.arm_pending_unit_ability(PendingUnitAbility::StealCashHack);
                return;
            }
            crate::command_system::CommandType::DisableVehicleHack { .. } => {
                self.arm_pending_unit_ability(PendingUnitAbility::DisableVehicleHack);
                return;
            }
            crate::command_system::CommandType::HackerDisableBuilding { .. } => {
                let selected = self.ui_selected_ids(self.current_player_id);
                if !self.host_control_bar_selection_has_ready_hacker_disable(&selected) {
                    return;
                }
                self.arm_pending_unit_ability(PendingUnitAbility::HackerDisableBuilding);
                return;
            }
            crate::command_system::CommandType::DisguiseAsVehicle { .. } => {
                self.arm_pending_unit_ability(PendingUnitAbility::DisguiseAsVehicle);
                return;
            }
            crate::command_system::CommandType::PlantBoobyTrap { .. } => {
                self.arm_pending_unit_ability(PendingUnitAbility::PlantBoobyTrap);
                return;
            }
            crate::command_system::CommandType::ConvertToCarbomb { .. } => {
                self.arm_pending_unit_ability(PendingUnitAbility::ConvertToCarbomb);
                return;
            }
            crate::command_system::CommandType::Repair { .. } => {
                self.arm_pending_unit_ability(PendingUnitAbility::Repair);
                return;
            }
            crate::command_system::CommandType::PurchaseScience { science_name } => {
                // C++ populatePurchaseScience / Alt+G opens the screen only.
                // Named science is the clicked purchase (GameLogicDispatch.cpp:1961).
                if science_name.trim().is_empty() {
                    self.try_purchase_next_generals_science();
                } else {
                    let player_id = self.current_player_id;
                    self.host_queue_command(crate::command_system::GameCommand {
                        command_type: crate::command_system::CommandType::PurchaseScience {
                            science_name: science_name.clone(),
                        },
                        player_id,
                        command_id: 0,
                        timestamp: std::time::SystemTime::now(),
                        selected_units: Vec::new(),
                        modifier_keys: crate::command_system::ModifierKeys::default(),
                    });
                }
                return;
            }
            crate::command_system::CommandType::ResumeConstruction { .. } => {
                self.resume_selected_construction();
                return;
            }
            crate::command_system::CommandType::Cheer => {
                // C++ MSG_META_ALL_CHEER only in multiplayer.
                if !self.host_is_in_multiplayer_game() {
                    return;
                }
                // C++ CommandXlat.cpp:3473 — play MiscAudio AllCheerSound
                // before appending MSG_DO_CHEER.
                play_all_cheer_sound();
            }
            crate::command_system::CommandType::RemoveBeacon => {
                if !self.host_command_xlat_multiplayer_meta() {
                    return;
                }
            }
            _ => {}
        }

        let mut command_type = command_type;
        // Prefer engine current player; fall back to lowest id residual.
        let player_id = self.local_player_id_for_ui();

        // Wave 219: selection via presentation-first ui_selected_ids.
        let selected = self.ui_selected_ids(player_id);
        if selected.is_empty()
            && !matches!(
                command_type,
                crate::command_system::CommandType::Stop
                    | crate::command_system::CommandType::Scatter
                    | crate::command_system::CommandType::ViewCommandCenter
                    | crate::command_system::CommandType::ViewLastRadarEvent
                    | crate::command_system::CommandType::PlaceBeacon { .. }
                    | crate::command_system::CommandType::RemoveBeacon
                    | crate::command_system::CommandType::SetBeaconText { .. }
                    | crate::command_system::CommandType::SelfDestruct { .. }
                    | crate::command_system::CommandType::EnableRetaliationMode { .. }
            )
        {
            return;
        }
        match &mut command_type {
            crate::command_system::CommandType::DozerCancelConstruct { object_id }
            | crate::command_system::CommandType::Sell { object_id } => {
                if let Some(id) = selected.first() {
                    *object_id = *id;
                }
            }
            crate::command_system::CommandType::ResumeConstruction { target_id } => {
                // Prefer unfinished structure in selection residual.
                let unfinished = selected
                    .iter()
                    .copied()
                    .find(|&id| self.ui_object_under_construction(id));
                if let Some(id) = unfinished.or_else(|| selected.first().copied()) {
                    *target_id = id;
                }
            }
            _ => {}
        }
        if named_command_is_single_use(command_name) {
            for id in &selected {
                if let Some(obj) = self.game_logic.host_object_mut(*id) {
                    obj.mark_single_use_command_used();
                }
            }
        }
        self.host_queue_and_process_command_silent(crate::command_system::GameCommand {
            command_type,
            player_id,
            command_id: 0,
            timestamp: std::time::SystemTime::now(),
            selected_units: selected,
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
    }
}

/// C++ ControlBarCommandProcessing.cpp:183-189.
fn play_named_command_button_unit_specific_sound(command_name: &str, player_id: u32) {
    let event_name = {
        let Some(bar) = game_engine::common::ini::ini_command_button::get_control_bar() else {
            return;
        };
        let Some(button) = bar.find_command_button_resolved(command_name) else {
            return;
        };
        let name = button.unit_specific_sound.playable_event_name();
        if name.is_empty() {
            return;
        }
        name.to_string()
    };
    if let Some(audio) = gamelogic::helpers::TheAudio::get() {
        let mut event = gamelogic::common::audio::AudioEventRts::new(&event_name);
        event.set_player_index(player_id);
        let _ = audio.add_audio_event(&event);
    } else {
        let _ = crate::assets::audio::play_sound_through_the_audio(&event_name);
    }
}

/// C++ ControlBarCommandProcessing.cpp:167-178.
fn named_command_is_single_use(command_name: &str) -> bool {
    let Some(bar) = game_engine::common::ini::ini_command_button::get_control_bar() else {
        return false;
    };
    let Some(button) = bar.find_command_button_resolved(command_name) else {
        return false;
    };
    button.options.one_shot
        || (button.options_bits
            & crate::game_logic::host_production_buildable_command_residual::COMMAND_OPTION_SINGLE_USE_COMMAND)
            != 0
}

/// C++ CommandXlat.cpp:3473 — `TheAudio->addAudioEvent(&m_allCheerSound)`.
fn play_all_cheer_sound() {
    if let Some(audio) = gamelogic::helpers::TheAudio::get() {
        let _ =
            audio.add_audio_event(&gamelogic::helpers::TheAudio::get_misc_audio().all_cheer_sound);
        return;
    }
    let name = game_engine::common::ini::ini_misc_audio::get_misc_audio()
        .map(|misc| {
            let misc = misc.read();
            let n = misc.all_cheer_sound.playable_event_name();
            if n.is_empty() {
                "UI_AllCheerSound".to_string()
            } else {
                n.to_string()
            }
        })
        .unwrap_or_else(|| "UI_AllCheerSound".to_string());
    let _ = crate::assets::audio::play_sound_through_the_audio(&name);
}

fn special_power_pending_options(power: &crate::command_system::SpecialPowerType) -> u32 {
    use crate::command_system::SpecialPowerType as P;
    const NEED_ENEMY: u32 = 0x0000_0001;
    const NEED_POS: u32 = 0x0000_0020;
    // Leftover ActionManager::can_do_special_power: no-option immediate fire.
    if crate::command_system::leftover_special_power_is_no_target(power) {
        return 0;
    }
    match power {
        P::BlackLotusStealCash
        | P::BlackLotusDisableVehicle
        | P::HackerDisableBuilding
        | P::MicrowaveDisableBuilding
        | P::CashHack
        | P::RangerCaptureBuilding
        | P::RedGuardCaptureBuilding
        | P::RebelCaptureBuilding
        | P::BlackLotusCaptureBuilding
        | P::DisguiseAsVehiclePower => NEED_ENEMY,
        _ => NEED_POS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attack_objects_position_weapon_uses_click_location_and_keeps_shot_budget() {
        let click = glam::Vec3::new(41.0, 0.0, -23.0);
        let command = resolve_pending_weapon_command(
            PendingWeaponCommand {
                weapon_slot: crate::command_system::WeaponSlot::Primary,
                max_shots_to_fire: 1,
                // C++ CommandOption::ATTACK_OBJECTS_POSITION.
                options: 0x0000_1000,
            },
            click,
            Some(crate::game_logic::ObjectId(77)),
        );

        assert_eq!(
            command,
            crate::command_system::CommandType::DoWeapon {
                weapon_slot: crate::command_system::WeaponSlot::Primary,
                max_shots_to_fire: 1,
                target: crate::command_system::WeaponTarget::Location(click),
            }
        );
    }

    #[test]
    fn leftover_no_target_special_powers_do_not_invent_need_pos() {
        use crate::command_system::SpecialPowerType as P;
        const NEED_POS: u32 = 0x0000_0020;
        for power in [
            P::CiaIntelligence,
            P::CommunicationsDownload,
            P::DetonateDirtyNuke,
            P::BurtonRemoteCharges,
            P::DemoKellRemoteCharges,
            P::BaikonurRocket,
            P::BattlePlanBombardment,
            P::BattlePlanHoldTheLine,
            P::BattlePlanSearchAndDestroy,
        ] {
            assert_eq!(
                special_power_pending_options(&power),
                0,
                "{power:?} leftover can_do_special_power is no-option"
            );
        }
        assert_eq!(
            special_power_pending_options(&P::SpySatellite) & NEED_POS,
            NEED_POS
        );
        assert_eq!(
            special_power_pending_options(&P::ParticleCannon) & NEED_POS,
            NEED_POS
        );
    }

    #[test]
    fn retail_combat_drop_keeps_object_click_precedence_over_its_position_option() {
        let click = glam::Vec3::new(41.0, 0.0, -23.0);
        let target_id = crate::game_logic::ObjectId(77);
        // Parsed Command_CombatDrop: enemy | neutral | ally | position |
        // multi-select | context command.
        let retail = PendingCombatDropCommand {
            options: 0x0000_0327,
        };

        assert_eq!(
            resolve_pending_combat_drop_command(retail.clone(), click, Some(target_id)),
            Some(crate::command_system::CommandType::CombatDrop {
                target: crate::command_system::DropTarget::Object(target_id),
            })
        );
        assert_eq!(
            resolve_pending_combat_drop_command(retail, click, None),
            Some(crate::command_system::CommandType::CombatDrop {
                target: crate::command_system::DropTarget::Location(click),
            })
        );
    }

    #[test]
    fn combat_drop_without_a_parsed_target_mode_fails_closed() {
        assert_eq!(
            resolve_pending_combat_drop_command(
                PendingCombatDropCommand { options: 0 },
                glam::Vec3::ZERO,
                Some(crate::game_logic::ObjectId(77)),
            ),
            None
        );
    }

    #[test]
    fn retail_superweapon_button_family_keeps_exact_parsed_variant_and_cursor() {
        use crate::command_system::{
            SpecialPowerType as Power, special_power_type_from_template_name,
        };

        let supw_particle =
            special_power_type_from_template_name("SupW_SuperweaponParticleUplinkCannon")
                .expect("retail SupW particle power");
        let laser_particle = special_power_type_from_template_name("Lazr_LaserCannon")
            .expect("retail Lazr particle power");
        let supw_neutron = special_power_type_from_template_name("SupW_SuperweaponNeutronMissile")
            .expect("retail SupW neutron power");

        // The shared visual button must return the exact selected module enum,
        // not its baseline representative.  That is what the command executor
        // uses to locate and spend the matching parsed module timer.
        assert_eq!(
            exact_parsed_structure_power_for_button(&Power::ParticleCannon, supw_particle.clone()),
            Some(supw_particle.clone())
        );
        assert_eq!(
            exact_parsed_structure_power_for_button(&Power::NuclearMissile, supw_neutron.clone()),
            Some(supw_neutron.clone())
        );

        assert!(parsed_structure_superweapon_matches_button(
            &Power::ParticleCannon,
            &laser_particle
        ));
        assert!(parsed_structure_superweapon_matches_button(
            &Power::NuclearMissile,
            &supw_neutron
        ));
        assert!(
            !parsed_structure_superweapon_matches_button(&Power::ParticleCannon, &supw_neutron),
            "a ready neutron missile must not satisfy a Particle Uplink button"
        );
        assert_eq!(
            exact_parsed_structure_power_for_button(&Power::ParticleCannon, supw_neutron.clone()),
            None,
            "a named Particle button must not fall through to an unrelated ready neutron module"
        );

        assert_eq!(
            CnCGameEngine::radius_cursor_type_for_special_power(&supw_particle),
            "PARTICLECANNON"
        );
        assert_eq!(
            CnCGameEngine::radius_cursor_type_for_special_power(&laser_particle),
            "PARTICLECANNON"
        );
        assert_eq!(
            CnCGameEngine::radius_cursor_type_for_special_power(&supw_neutron),
            "NUCLEARMISSILE"
        );
    }

    #[test]
    fn max_selection_warning_substitutes_count_not_raw_key() {
        // C++ SelectionXlat.cpp:283-290 / InGameUI.cpp:120-127
        // TheGameText->fetch("GUI:MaxSelectionSize").format(count)
        let msg = format_max_selection_size_message(25);
        assert!(
            !msg.starts_with("GUI:MaxSelectionSize"),
            "live warning must not show the raw GameText key, got {msg:?}"
        );
        assert!(
            msg.contains("25"),
            "localized warning must include the cap, got {msg:?}"
        );
    }

    #[test]
    fn all_cheer_sound_uses_retail_misc_audio_event() {
        // Retail MiscAudio.ini: AllCheerSound = UI_AllCheerSound.
        let handle = game_engine::common::ini::ini_misc_audio::ensure_misc_audio();
        {
            let mut misc = handle.write();
            misc.all_cheer_sound =
                game_engine::common::ini::ini_misc_audio::AudioEventRTS::from_event_name(
                    "UI_AllCheerSound".to_string(),
                );
        }
        play_all_cheer_sound();
        assert_eq!(
            gamelogic::helpers::TheAudio::get_misc_audio()
                .all_cheer_sound
                .get_event_name(),
            "UI_AllCheerSound"
        );
    }

    #[test]
    fn command_button_unit_specific_sound_resolves_from_ini() {
        game_engine::common::ini::ini_command_button::initialize_control_bar();
        {
            let mut bar = game_engine::common::ini::ini_command_button::get_control_bar_mut()
                .expect("INI control bar");
            let button = bar.new_command_button("Command_BlackLotusHackBuilding".to_string());
            button.unit_specific_sound =
                game_engine::common::ini::ini_misc_audio::AudioEventRTS::from_event_name(
                    "BlackLotusVoiceModeBuilding".to_string(),
                );
        }
        play_named_command_button_unit_specific_sound("Command_BlackLotusHackBuilding", 0);
        let bar = game_engine::common::ini::ini_command_button::get_control_bar().unwrap();
        let button = bar
            .find_command_button_resolved("Command_BlackLotusHackBuilding")
            .unwrap();
        assert_eq!(
            button.unit_specific_sound.playable_event_name(),
            "BlackLotusVoiceModeBuilding"
        );
    }

    #[test]
    fn leftover_radius_cursor_attack_ground_is_not_table_zero() {
        assert_eq!(
            leftover_store_radius_cursor_for_type("OFFENSIVE_SPECIALPOWER"),
            0.0
        );
        let src = include_str!("ui_commands.rs");
        assert!(
            src.contains("PendingMapCommand::Weapon(_) => \"ATTACK_DAMAGE_AREA\""),
            "FIRE_WEAPON / attack-ground must arm leftover ATTACK_DAMAGE_AREA"
        );
        assert!(
            !src.contains("o.weapon_range") && !src.contains("o.vision_range"),
            "must not proxy radius from presentation weapon/vision"
        );
        assert!(
            !src.contains("Attack-move: click target location")
                && !src.contains("Guard: click location or unit")
                && !src.contains("Set rally point: click location")
                && !src.contains("Combat drop: click landing zone")
                && !src.contains("Special power: click target location")
                && !src.contains("No ready special power on selection")
                && !src.contains("Place beacon: click location")
                && !src.contains(concat!("Select a valid", " target"))
                && !src.contains(concat!("Canceled all", " production")),
            "command arming/cancel must not invent HUD instruction toasts"
        );
    }
}
