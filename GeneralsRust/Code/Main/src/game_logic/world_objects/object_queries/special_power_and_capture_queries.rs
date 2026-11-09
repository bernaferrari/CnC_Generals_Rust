use super::*;

impl GameLogic {
    /// Combined object + SharedSyncedTimer + RequiredScience residual ready gate.
    ///
    /// C++ order residual: object alive → science (`canUseSpecialPower`) →
    /// sharedNSync / per-object cooldown.
    pub fn is_special_power_ready_for(
        &self,
        object_id: ObjectId,
        power: &crate::command_system::SpecialPowerType,
    ) -> bool {
        let Some(obj) = self.host_object(object_id) else {
            return false;
        };
        if !obj.is_alive() {
            return false;
        }
        // C++ Player.cpp:1240-1304 `doFindSpecialPowerSourceObject` /
        // `doCountSpecialPowersReady` refuse UNDER_CONSTRUCTION and SOLD.
        // `is_disabled()` already covers UC, but sell clears UC and keeps
        // the object alive — sold must be an explicit fire skip.
        if obj.status.under_construction || obj.status.sold {
            return false;
        }
        // C++ SpecialPowerModule::doSpecialPower / isReady: disabled objects cannot fire.
        // Covers underpowered POWERED SWs (PUC/Nuke), EMP, hacked, unmanned, etc.
        if obj.is_disabled() {
            return false;
        }
        // C++ SpecialPower.cpp:308 — no any-unit fallback when the object
        // does not carry this SpecialPowerModule (Frenzy/CashHack included).
        if !object_has_special_power_module(obj, power) {
            return false;
        }
        let parsed_module = obj.thing().template.special_power_module_for_command(power);
        // A parsed module's loaded SpecialPowerTemplate owns science and
        // SharedSyncedTimer policy.  Fall back only for command families not
        // yet represented by a source module record.
        let required_science = parsed_module
            .and_then(|module| module.required_science.as_deref())
            .or_else(|| {
                (parsed_module.is_none()).then(|| {
                    crate::game_logic::host_special_power_enum_residual::special_power_required_science(
                        power,
                    )
                })
                .flatten()
            });
        if let Some(required) = required_science {
            // C++ SpecialPowerModule.cpp:278/323 getControllingPlayer, not first
            // same-faction slot. Mirror 1v1 USA/USA must not share science.
            match self.controlling_player_for_special_power(obj) {
                Some(player) if player.has_unlocked_science(required) => {}
                Some(_) => return false,
                // Fail-closed: science-gated powers need a controlling player residual.
                None => return false,
            }
        }
        let shared_n_sync = parsed_module
            .map(|module| module.shared_n_sync)
            .unwrap_or_else(|| {
                crate::game_logic::host_special_power_enum_residual::special_power_uses_shared_synced_timer(
                    power,
                )
            });
        if shared_n_sync {
            // C++ doSpecialPower refuses a paused module independently of
            // SharedNSync; isReady only compares the player timer.
            if obj.is_special_power_countdown_paused(power) {
                return false;
            }
            // C++ getReadyFrame via Player::getOrStartSpecialPowerReadyFrame.
            if let Some(player) = self.controlling_player_for_special_power(obj) {
                if let Some(module) = parsed_module {
                    if player.authored_special_power_timers.get(&module.special_power_template.to_ascii_lowercase()).copied().unwrap_or(0.0) > 0.0 {
                        return false;
                    }
                } else if !player.is_shared_special_power_ready(power) {
                    return false;
                }
            }
            return true;
        }
        obj.is_special_power_ready(power)
    }

    /// Consume charge with SharedSyncedTimer residual when applicable.
    pub fn consume_special_power_charge_for(
        &mut self,
        object_id: ObjectId,
        power: &crate::command_system::SpecialPowerType,
    ) -> bool {
        if !self.is_special_power_ready_for(object_id, power) {
            return false;
        }
        let (owner_id, team, parsed_module) = match self.host_object(object_id) {
            Some(o) => (
                self.player_owner_for_host_object(o),
                o.team,
                o.thing()
                    .template
                    .special_power_module_for_command(power)
                    .cloned(),
            ),
            None => return false,
        };
        // Cooldown-tracked objects without a parsed module record consume via
        // the residual reload table below (C++ SpecialPowerModule::isReady /
        // startPowerRecharge read the live timer, not a parsed INI record).
        // Only objects with no module evidence at all fail closed — and that
        // case already returned false inside is_special_power_ready_for.
        let reload = parsed_module
            .as_ref()
            .map(|module| module.reload_time_frames as f32 / 30.0)
            .unwrap_or_else(|| {
                crate::game_logic::host_special_power_enum_residual::special_power_reload_seconds(
                    power,
                )
                .unwrap_or_else(|| {
                    self.host_object(object_id)
                        .map(|o| o.special_power_cooldown)
                        .unwrap_or(10.0)
                })
            });

        let shared_n_sync = parsed_module
            .as_ref()
            .map(|module| module.shared_n_sync)
            .unwrap_or_else(|| {
                crate::game_logic::host_special_power_enum_residual::special_power_uses_shared_synced_timer(
                    power,
                )
            });
        let frame = self.frame;
        if shared_n_sync {
            if let Some(module) = parsed_module.as_ref() {
                if let Some(player) = owner_id.and_then(|id| self.get_player_mut(id)) {
                    let key = module.special_power_template.to_ascii_lowercase();
                    player.authored_special_power_timers.insert(key, reload);
                }
                return true;
            }
            if let Some(pid) = owner_id {
                if let Some(player) = self.get_player_mut(pid) {
                    player.reset_shared_special_power_timer(power, reload);
                }
            }
            // Mirror onto living objects owned by the same controlling player.
            // C++ SharedNSync is per-Player, not per-faction.
            for obj in self.objects.registered_values_mut() {
                let same_controller = match owner_id {
                    Some(want) => {
                        obj.owner_player_id == Some(want)
                            || (obj.owner_player_id.is_none() && obj.team == team)
                    }
                    None => obj.team == team && obj.owner_player_id.is_none(),
                };
                if !same_controller || !obj.is_alive() {
                    continue;
                }
                if reload > 0.0 {
                    obj.special_power_cooldowns.insert(power.clone(), reload);
                } else {
                    obj.special_power_cooldowns.remove(power);
                }
                obj.refresh_special_power_aggregate_cooldown();
            }
        } else if let Some((obj, audio_events)) = self.host_object_and_audio_events_mut(object_id) {
            if let Some(module) = parsed_module.as_ref() {
                obj.start_power_recharge_with_frames(power, module.reload_time_frames);
                obj.set_ai_state(
                    AIState::Idle,
                    frame,
                    &mut crate::game_logic::ObjectAudio::queue(audio_events),
                );
            } else {
                obj.consume_special_power_charge(
                    power,
                    frame,
                    &mut crate::game_logic::ObjectAudio::queue(audio_events),
                );
            }
        }
        true
    }

    /// C++ `Object::getControllingPlayer` residual for science / SharedNSync.
    pub(super) fn controlling_player_for_special_power(&self, obj: &Object) -> Option<&Player> {
        let owner_id = self.player_owner_for_host_object(obj)?;
        self.players.get(&owner_id)
    }

    /// C++ `ScriptActions::doNamedStopSpecialPowerCountdown` residual.
    pub fn script_pause_special_power_countdown(
        &mut self,
        object_id: ObjectId,
        power: &crate::command_system::SpecialPowerType,
        pause: bool,
    ) -> bool {
        let Some(obj) = self.host_object_mut(object_id) else {
            return false;
        };
        obj.pause_special_power_countdown(power, pause);
        true
    }

    /// C++ `ScriptActions::doNamedSetSpecialPowerCountdown` residual.
    pub fn script_set_special_power_countdown(
        &mut self,
        object_id: ObjectId,
        power: &crate::command_system::SpecialPowerType,
        seconds: i32,
    ) -> bool {
        let Some(obj) = self.host_object_mut(object_id) else {
            return false;
        };
        obj.set_special_power_ready_seconds(power, seconds.max(0) as f32);
        true
    }

    /// C++ `ScriptActions::doNamedAddSpecialPowerCountdown` residual.
    pub fn script_add_special_power_countdown(
        &mut self,
        object_id: ObjectId,
        power: &crate::command_system::SpecialPowerType,
        seconds: i32,
    ) -> bool {
        let Some(obj) = self.host_object(object_id) else {
            return false;
        };
        let remaining = obj.thing().template.special_power_module_for_command(power)
            .filter(|module| module.shared_n_sync)
            .and_then(|module| self.controlling_player_for_special_power(obj).map(|player|
                player.authored_special_power_timers.get(&module.special_power_template.to_ascii_lowercase()).copied().unwrap_or(0.0)))
            .unwrap_or_else(|| obj.special_power_countdown_seconds(power));
        let next = remaining + seconds as f32;
        let Some(obj) = self.host_object_mut(object_id) else { return false };
        obj.set_special_power_ready_seconds(power, next.max(0.0));
        true
    }

    /// CPP ScriptEngine.cpp5975-5991: resolve the literal name in this
    /// session's live objects. THIS_OBJECT belongs to the supplied script context.
    pub fn host_object_id_by_script_name(&self, unit_name: &str) -> Option<ObjectId> {
        let target = self.named_script_cache.lookup(unit_name)?;
        self.objects
            .get(&target.id)
            .filter(|object| target.matches(object.id, &object.visual_draw_identity_token))
            .map(|object| object.id)
    }

    /// Apply NAMED_STOP/START/SET/ADD_SPECIAL_POWER_COUNTDOWN to a named host object.
    pub fn script_named_special_power_countdown(
        &mut self,
        unit_name: &str,
        power_name: &str,
        op: crate::game_logic::NamedSpecialPowerCountdownOp,
        seconds: i32,
    ) -> bool {
        use crate::command_system::special_power_type_from_template_name;
        use crate::game_logic::NamedSpecialPowerCountdownOp;
        let Some(object_id) = self.host_object_id_by_script_name(unit_name) else {
            return false;
        };
        let authored = self.host_object(object_id).and_then(|object| object.thing().template.special_power_modules.iter()
            .find(|module| module.special_power_template.eq_ignore_ascii_case(power_name))
            .map(|module| {
                let shared_remaining = module.shared_n_sync.then(|| self.controlling_player_for_special_power(object)
                    .map(|player| player.authored_special_power_timers.get(&module.special_power_template.to_ascii_lowercase()).copied().unwrap_or(0.0)))
                    .flatten();
                (module.source_index, shared_remaining)
            }));
        if let Some((index, shared_remaining)) = authored {
            let Some(object) = self.host_object_mut(object_id) else { return false };
            let state = object.authored_special_power_timers.entry(index).or_default();
            match op {
                NamedSpecialPowerCountdownOp::Stop => state.1 = state.1.saturating_add(1),
                NamedSpecialPowerCountdownOp::Start => state.1 = state.1.saturating_sub(1),
                NamedSpecialPowerCountdownOp::Set => state.0 = seconds.max(0) as f32,
                NamedSpecialPowerCountdownOp::Add => state.0 = (shared_remaining.unwrap_or(state.0) + seconds as f32).max(0.0),
            }
            object.refresh_special_power_aggregate_cooldown();
            return true;
        }
        let Some(power) = special_power_type_from_template_name(power_name) else {
            return false;
        };
        // An enum adapter must not redirect an authored template name to a
        // different module that happens to share its command family.
        if self.host_object(object_id).is_some_and(|object|
            object.thing().template.special_power_module_for_command(&power).is_some()) {
            return false;
        }
        match op {
            NamedSpecialPowerCountdownOp::Stop => {
                self.script_pause_special_power_countdown(object_id, &power, true)
            }
            NamedSpecialPowerCountdownOp::Start => {
                self.script_pause_special_power_countdown(object_id, &power, false)
            }
            NamedSpecialPowerCountdownOp::Set => {
                self.script_set_special_power_countdown(object_id, &power, seconds)
            }
            NamedSpecialPowerCountdownOp::Add => {
                self.script_add_special_power_countdown(object_id, &power, seconds)
            }
        }
    }

    /// Tick all players' SharedSyncedTimer residual cooldowns.
    ///
    /// Fires EVA SuperweaponReady residual when a PublicTimer power finishes
    /// recharging (own/ally/enemy classification via try_eva_superweapon_ready).
    pub fn tick_shared_special_power_timers(&mut self, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        let mut authored_ready = Vec::new();
        let mut ready_events: Vec<(u32, String)> = Vec::new();
        for player in self.players.values_mut() {
            let player_id = player.id;
            for (template, remaining) in player.authored_special_power_timers.iter_mut() {
                let was = *remaining;
                *remaining = (*remaining - dt).max(0.0);
                if was > 0.0 && *remaining <= 0.0 {
                    authored_ready.push((player_id, template.clone()));
                }
            }
            for power in player.tick_shared_special_power_timers(dt) {
                use crate::game_logic::host_special_power_enum_residual::special_power_has_public_timer;
                if !special_power_has_public_timer(&power) {
                    continue;
                }
                // Map power → structure template name residual for EVA classifier.
                let template = match power {
                    crate::command_system::SpecialPowerType::ParticleCannon
                    | crate::command_system::SpecialPowerType::SuperweaponParticleCannon
                    | crate::command_system::SpecialPowerType::LaserCannon => {
                        "AmericaParticleCannonUplink"
                    }
                    crate::command_system::SpecialPowerType::NuclearMissile
                    | crate::command_system::SpecialPowerType::NukeNeutronMissile
                    | crate::command_system::SpecialPowerType::SuperweaponNeutronMissile
                    | crate::command_system::SpecialPowerType::BaikonurRocket => {
                        "ChinaNuclearMissileLauncher"
                    }
                    crate::command_system::SpecialPowerType::ScudStorm => "GLAScudStorm",
                    _ => continue, // EVA only for PUC/Nuke/Scud residual family
                };
                ready_events.push((player_id, template.to_string()));
            }
        }
        for (player_id, template) in authored_ready {
            let source = self.objects.registered_values().find(|obj|
                self.player_owner_for_host_object(obj) == Some(player_id)
                    && obj.thing().template.special_power_modules.iter().any(|module|
                        module.shared_n_sync && module.public_timer
                            && module.special_power_template.eq_ignore_ascii_case(&template)))
                .map(|obj| obj.id);
            if let Some(source) = source {
                self.try_eva_superweapon_ready_for_source(source);
            }
        }
        for (player_id, name) in ready_events {
            self.try_eva_superweapon_ready_for_player(player_id, &name);
        }
    }

    /// Wake every matching behavior on objects controlled by the receiving player.
    pub fn on_special_power_science_creation(&mut self, player_id: u32, science_name: &str) {
        use crate::game_logic::host_sp_science_upgrade_player_team_residual_wave109::normalize_science_name_residual;
        if !self.players.contains_key(&player_id) {
            return;
        }
        let science = normalize_science_name_residual(science_name);
        if science.is_empty() {
            return;
        }
        let matches: Vec<_> = self.objects.registered_values()
            .filter(|object| self.player_owner_for_host_object(object) == Some(player_id))
            .flat_map(|object| object.thing().template.special_power_modules.iter()
                .filter(|module| module.required_science.as_deref().is_some_and(|required|
                    normalize_science_name_residual(required).eq_ignore_ascii_case(&science)))
                .map(move |module| (object.id, module.source_index, module.special_power_template.to_ascii_lowercase(),
                    module.shared_n_sync, module.starts_paused, module.public_timer,
                    object.is_kind_of(KindOf::Structure))))
            .collect();
        for (object_id, index, template, shared, starts_paused, public, structure) in matches {
            if shared {
                if let Some(player) = self.players.get_mut(&player_id) {
                    player.authored_special_power_timers.insert(template, 0.0);
                }
            }
            if let Some(object) = self.objects.get_mut(&object_id) {
                let state = object.authored_special_power_timers.entry(index).or_default();
                state.0 = 0.0;
                if starts_paused {
                    state.1 = state.1.saturating_add(1);
                }
                if public && structure {
                    object.authored_public_timer_registered.insert(index);
                }
                object.refresh_special_power_aggregate_cooldown();
            }
        }
        self.apply_cash_bounty_for_added_science(player_id, science_name);
    }

    pub fn team_has_completed_capture_upgrade(&self, team: Team) -> bool {
        let Some(player) = self.players.values().find(|player| player.team == team) else {
            return true;
        };
        capture_upgrade_names_for_team(team)
            .iter()
            .any(|upgrade| player.has_unlocked_upgrade(upgrade))
    }

    /// C++ `ActionManager::canCaptureBuilding` authority slice.
    ///
    /// This is intentionally shared by physical RMB classification, command
    /// execution, and the in-progress capture state.  It retains the exact
    /// Object INI semantic inputs (SpecialPower, CAPTURABLE,
    /// IMMUNE_TO_CAPTURE, GarrisonContain) rather than deriving permission
    /// from Infantry/Hero/template spelling.
    pub fn can_unit_capture_building(
        &self,
        source_id: ObjectId,
        target_id: ObjectId,
        require_power_ready: bool,
    ) -> bool {
        use gamelogic::common::Relationship;

        if source_id == target_id {
            return false;
        }
        let Some(source) = self.objects.get(&source_id) else {
            return false;
        };
        let Some(target) = self.objects.get(&target_id) else {
            return false;
        };

        let Some(power) = source.thing().template.capture_power.special_power_type() else {
            return false;
        };
        if !source.is_alive()
            || !source.can_move()
            || !target.is_alive()
            || !target.is_kind_of(KindOf::Structure)
            || target.status.under_construction
            || target.status.sold
            || target.thing().template.immune_to_capture
            // C++ ActionManager rejects a pure-stealth target before the
            // relationship/capturable tests.  A disguise is intentionally
            // not pure stealth for this purpose.
            || (target.status.stealthed && !target.status.detected && !target.status.disguised)
        {
            return false;
        }
        if require_power_ready && !self.is_special_power_ready_for(source_id, &power) {
            return false;
        }

        if require_power_ready {
            if let Some(owner_id) = self.player_owner_for_host_object(source) {
                if self
                    .players
                    .get(&owner_id)
                    .is_some_and(|_| self.is_local_player(owner_id))
                {
                    let shroud_manager = self.world_services.shroud();
                    let visible = shroud_manager
                        .lock()
                        .map(|shroud| shroud.can_see_object(owner_id, target_id.0))
                        .unwrap_or(false);
                    if !visible {
                        return false;
                    }
                }
            }
        }

        // Player ownership is authoritative whenever both objects have it.
        // Map/unowned objects keep the old faction/Neutral fallback rather
        // than being fabricated into a player relation.
        let relation = if self.has_object_ownership_provenance(source, target) {
            self.object_relationship(source, target)
        } else if source.team == target.team {
            Relationship::Allies
        } else if source.team == Team::Neutral || target.team == Team::Neutral {
            Relationship::Neutral
        } else {
            Relationship::Enemies
        };
        if !(relation == Relationship::Enemies
            || (target.thing().template.capturable && relation != Relationship::Allies))
        {
            return false;
        }

        // C++ rejects a garrisonable target containing any non-stealthed
        // occupant, and *separately* rejects an apparent friendly occupant.
        // The latter is not conditional on a GarrisonContain module, so do
        // not accidentally authorize a structure whose runtime occupant list
        // still contains a friendly child. Missing objects are conservatively
        // rejected so stale links cannot make a defended building capturable.
        let target_is_garrisonable = target.thing().template.garrison_contain_max.is_some();
        for contained_id in target.contained_units() {
            let Some(contained) = self.objects.get(&contained_id) else {
                return false;
            };
            if target_is_garrisonable && !contained.status.stealthed {
                return false;
            }
            let contained_relation = if self.has_object_ownership_provenance(source, contained) {
                self.object_relationship(source, contained)
            } else if source.team == contained.team {
                Relationship::Allies
            } else if source.team == Team::Neutral || contained.team == Team::Neutral {
                Relationship::Neutral
            } else {
                Relationship::Enemies
            };
            if contained_relation == Relationship::Allies {
                return false;
            }
        }
        true
    }

    /// Exact readiness gate for a parsed Hacker Disable Building
    /// `SpecialAbility` module.  This deliberately bypasses the old global
    /// command-enum residual table: C++ owns HDB reload/science/shared state
    /// in the loaded SpecialPowerTemplate paired with this object.
    pub fn is_hacker_disable_building_ready(&self, source_id: ObjectId) -> bool {
        let Some(source) = self.objects.get(&source_id) else {
            return false;
        };
        let Some(metadata) = source.thing().template.hacker_disable_building.as_ref() else {
            return false;
        };
        let power = metadata.command_power();
        if !metadata.update_module_starts_attack
            || metadata.scripted_special_power_only
            || !source.is_alive()
            || source.is_disabled()
            || source.is_special_power_countdown_paused(&power)
        {
            return false;
        }

        let owner_id = self.player_owner_for_host_object(source);
        if let Some(required_science) = metadata.required_science.as_deref() {
            let Some(owner_id) = owner_id else {
                return false;
            };
            if !self
                .players
                .get(&owner_id)
                .is_some_and(|player| player.has_unlocked_science(required_science))
            {
                return false;
            }
        }
        if metadata.shared_n_sync {
            let Some(owner_id) = owner_id else {
                return false;
            };
            return self.players.get(&owner_id).is_some_and(|player|
                player.authored_special_power_timers.get(&metadata.special_power_template.to_ascii_lowercase()).copied().unwrap_or(0.0) <= 0.0);
        }
        source.is_special_power_ready(&power)
    }

    /// Start the exact parsed HDB reload at C++
    /// `SpecialAbilityUpdate::startPreparation`, not at click time.
    pub fn consume_hacker_disable_building_charge(&mut self, source_id: ObjectId) -> bool {
        if !self.is_hacker_disable_building_ready(source_id) {
            return false;
        }
        let Some((metadata, owner_id)) = self.objects.get(&source_id).and_then(|source| {
            source
                .thing()
                .template
                .hacker_disable_building
                .clone()
                .map(|metadata| (metadata, self.player_owner_for_host_object(source)))
        }) else {
            return false;
        };
        let power = metadata.command_power();
        if metadata.shared_n_sync {
            let Some(owner_id) = owner_id else {
                return false;
            };
            let Some(player) = self.players.get_mut(&owner_id) else {
                return false;
            };
            let key = metadata.special_power_template.to_ascii_lowercase();
            // The readiness query above is C++ getOrStart: it creates a ready
            // entry before startPowerRecharge resets it.
            player.authored_special_power_timers.insert(key, metadata.reload_time_frames as f32 / 30.0);
        } else if let Some(source) = self.objects.get_mut(&source_id) {
            source.start_power_recharge_with_frames(&power, metadata.reload_time_frames);
        } else {
            return false;
        }
        true
    }

    /// C++ `ActionManager::canDisableBuildingViaHacking` authority slice.
    ///
    /// Both executor and the in-progress HDB channel call this typed path.
    /// `require_power_ready=false` preserves a channel that has already begun
    /// its authored recharge; it still revalidates all live source/target
    /// semantics without treating an expired cooldown as a cancellation.
    pub fn can_unit_hacker_disable_building(
        &self,
        source_id: ObjectId,
        target_id: ObjectId,
        require_power_ready: bool,
    ) -> bool {
        use gamelogic::common::Relationship;

        if source_id == target_id {
            return false;
        }
        let Some(source) = self.objects.get(&source_id) else {
            return false;
        };
        let Some(target) = self.objects.get(&target_id) else {
            return false;
        };
        let Some(metadata) = source.thing().template.hacker_disable_building.as_ref() else {
            return false;
        };
        if !metadata.update_module_starts_attack
            || metadata.scripted_special_power_only
            || !source.is_alive()
            || source.is_disabled()
            || !target.is_alive()
            || !target.is_kind_of(KindOf::Structure)
            || target.is_rebuild_hole
            || target.status.under_construction
            || target.is_effectively_stealthed()
        {
            return false;
        }
        if require_power_ready && !self.is_hacker_disable_building_ready(source_id) {
            return false;
        }

        // `isObjectShroudedForAction` applies to human click authority, not
        // an already running update.  Main's local player is the exact host
        // representation of C++ PLAYER_HUMAN; stale/poisoned visibility fails
        // closed rather than turning an unseen enemy into a valid target.
        if require_power_ready {
            if let Some(owner_id) = self.player_owner_for_host_object(source) {
                if self
                    .players
                    .get(&owner_id)
                    .is_some_and(|_| self.is_local_player(owner_id))
                {
                    let shroud_manager = self.world_services.shroud();
                    let visible = shroud_manager
                        .lock()
                        .map(|shroud| shroud.can_see_object(owner_id, target_id.0))
                        .unwrap_or(false);
                    if !visible {
                        return false;
                    }
                }
            }
        }

        // C++ ActionManager SPECIAL_HACKER_DISABLE_BUILDING reads
        // `obj->getRelationship(target)` (Object.cpp:1548-1568), which falls
        // through to the controlling players' team relationship: two living
        // players on different non-neutral teams are ENEMIES even when the
        // lobby carries no explicit playerEnemies row. A strict player-map
        // NEUTRAL must not hard-reject that default hostility.
        let mut relation = match (source.owner_player_id, target.owner_player_id) {
            (Some(source_owner), Some(target_owner))
                if self.player_owner_for_host_object(source) == Some(source_owner)
                    && self.player_owner_for_host_object(target) == Some(target_owner) =>
            {
                self.player_relationship(source_owner, target_owner)
            }
            (None, None) if self.uses_legacy_team_ownership_fallback() => {
                if source.team == target.team {
                    Relationship::Allies
                } else if source.team == Team::Neutral || target.team == Team::Neutral {
                    Relationship::Neutral
                } else {
                    Relationship::Enemies
                }
            }
            _ => Relationship::Neutral,
        };
        if relation == Relationship::Neutral
            && source.team != Team::Neutral
            && target.team != Team::Neutral
            && source.team != target.team
        {
            relation = Relationship::Enemies;
        }
        if relation != Relationship::Enemies {
            return false;
        }

        // C++ permits either a normal capturable building or its FSTechnology
        // exception.  The exception has to remain separate: a technology
        // structure is legal even if it lacks CAPTURABLE, but not if immune.
        let capturable = target.thing().template.capturable && !target.is_rebuild_hole;
        let technology_exception =
            target.is_kind_of(KindOf::FSTechnology) && !target.thing().template.immune_to_capture;
        if !(capturable || technology_exception) {
            return false;
        }

        // `appearsToContainFriendlies` is independent of GarrisonContain.
        // A stale contained id is unknown to C++; fail closed rather than
        // making malformed containment look empty and hackable.
        for contained_id in target.contained_units() {
            let Some(contained) = self.objects.get(&contained_id) else {
                return false;
            };
            let contained_relation = match (source.owner_player_id, contained.owner_player_id) {
                (Some(source_owner), Some(contained_owner))
                    if self.player_owner_for_host_object(source) == Some(source_owner)
                        && self.player_owner_for_host_object(contained)
                            == Some(contained_owner) =>
                {
                    self.player_relationship(source_owner, contained_owner)
                }
                (None, None) if self.uses_legacy_team_ownership_fallback() => {
                    if source.team == contained.team {
                        Relationship::Allies
                    } else if source.team == Team::Neutral || contained.team == Team::Neutral {
                        Relationship::Neutral
                    } else {
                        Relationship::Enemies
                    }
                }
                _ => Relationship::Neutral,
            };
            if contained_relation == Relationship::Allies {
                return false;
            }
        }
        true
    }

    /// Exact Object INI capture range for an already-issued capture order.
    /// A missing `SpecialAbilityUpdate` fails closed rather than silently
    /// granting the old generic infantry/hero radius.
    pub fn unit_capture_start_ability_range(&self, object_id: ObjectId) -> Option<f32> {
        self.objects
            .get(&object_id)
            .and_then(|obj| obj.thing().template.capture_start_ability_range)
    }

    pub fn local_player_id(&self) -> Option<u32> {
        self.local_player_id
            .filter(|id| self.players.contains_key(id))
    }

    pub fn is_local_player(&self, player_id: u32) -> bool {
        self.local_player_id() == Some(player_id)
    }

    /// Override a player's display name (used by CLI / networking parity).
    pub fn set_player_name(&mut self, player_id: u32, name: &str) -> bool {
        if let Some(player) = self.players.get_mut(&player_id) {
            player.name = name.to_string();
            true
        } else {
            false
        }
    }

    /// Override a player's team/faction at runtime (used by menu selection).
    pub fn set_player_team(&mut self, player_id: u32, team: Team) -> bool {
        if let Some(player) = self.players.get_mut(&player_id) {
            player.team = team;
            true
        } else {
            false
        }
    }

    /// Wave 921: single authority boundary for match start + local faction (+ optional AI).
    #[inline]
    pub fn start_new_game_with_faction(
        &mut self,
        mode: GameMode,
        player_id: u32,
        faction_team: Team,
        setup_skirmish_ai: bool,
    ) -> Result<(), String> {
        self.start_new_game(mode)?;
        let _ = self.set_player_team(player_id, faction_team);
        if setup_skirmish_ai {
            self.setup_skirmish_ai(player_id);
        }
        Ok(())
    }

    /// C++ GameInfo/SidesList start path for a selected Campaign or Challenge
    /// PlayerTemplate.  The exact identity is resolved before any base-team
    /// behavior is chosen; an invalid late resolution removes the local player
    /// instead of starting the wrong General.
    #[inline]
    pub fn start_new_game_with_player_template(
        &mut self,
        mode: GameMode,
        player_id: u32,
        player_template: PlayerTemplateIdentity,
    ) -> Result<bool, String> {
        self.start_new_game(mode)?;
        if !self.bind_player_template_identity(player_id, player_template) {
            log::error!(
                "Rejecting selected PlayerTemplate after session reset; removing local player {} rather than falling back to a base Team",
                player_id
            );
            self.players.remove(&player_id);
            if self.local_player_id == Some(player_id) {
                self.select_local_player(None);
            }
            self.player_template_bindings.remove(&player_id);
            return Ok(false);
        }
        Ok(true)
    }

    /// Bind an already validated offline GameInfo slot selection to an
    /// existing bootstrap player. Campaign/Challenge and offline Skirmish
    /// share this exact C++ `Player::init(PlayerTemplate)` seam; callers
    /// remain responsible for restoring any GameInfo dict overrides (such as
    /// the per-slot color) after this template-owned state is applied.
    pub(crate) fn bind_player_template_identity(
        &mut self,
        player_id: u32,
        player_template: PlayerTemplateIdentity,
    ) -> bool {
        self.with_own_services_mut(|this| {
            this.bind_player_template_identity_with_services(player_id, player_template)
        })
    }

    pub(super) fn bind_player_template_identity_with_services(
        &mut self,
        player_id: u32,
        player_template: PlayerTemplateIdentity,
    ) -> bool {
        let Some(template) = self.resolve_player_template_identity(&player_template) else {
            return false;
        };
        let Some(team) = PlayerTemplateIdentity::team_for_template(&template) else {
            return false;
        };
        let Some(player) = self.players.get_mut(&player_id) else {
            return false;
        };

        // C++ Player::init(pt): the exact template owns base side, preferred
        // color, starting money fallback, and rank-one sciences.  The generic
        // player seeded by start_new_game is only a bootstrap shell.
        player.team = team;
        player.resources.supplies = self.skirmish_rules.starting_cash;
        player.apply_player_template_start_state(&template);
        self.player_template_bindings
            .insert(player_id, player_template);
        true
    }
}
