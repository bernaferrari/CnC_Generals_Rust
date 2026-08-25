//! Host objects `impl GameLogic` — `host_ops_writeback`.
//! commands, host unmapped damage, host_object, writeback. Child of `world_objects` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    /// Select objects for a player (full replace).
    ///
    /// C++ `GameLogic::selectObject` (`GameLogic.cpp:2595-2641`) has no owner
    /// filter — `playerMask` is the only player predicate. Host `player_id`
    /// maps to that mask bit (`1 << player_id`).
    pub fn select_objects(&mut self, player_id: u32, object_ids: Vec<ObjectId>) {
        if self.players.get(&player_id).is_none() {
            return;
        }
        // C++ pickAndPlay VoiceSelect — queue_audio_event in select_object_list.
        self.select_object_list_ex(1u32 << player_id.min(31), object_ids, true, true);
    }

    /// C++ `MSG_CREATE_SELECTED_GROUP_NO_SOUND` / `MSG_ADD_TEAM` /
    /// `MSG_REMOVE_FROM_SELECTED_GROUP` — same select writeback, no VoiceSelect.
    pub fn select_objects_no_sound(&mut self, player_id: u32, object_ids: Vec<ObjectId>) {
        if self.players.get(&player_id).is_none() {
            return;
        }
        self.select_object_list_ex(1u32 << player_id.min(31), object_ids, true, false);
    }

    /// C++ `GameLogic::selectObject` over a host id list.
    ///
    /// `create_new_selection` is C++ `createNewSelection`: false rejects
    /// `!isMassSelectable()` (`Object.cpp:3024`) and appends; true replaces
    /// each player in `player_mask`. No owner-player filter.
    pub fn select_object_list(
        &mut self,
        player_mask: u32,
        object_ids: Vec<ObjectId>,
        create_new_selection: bool,
    ) {
        self.select_object_list_ex(player_mask, object_ids, create_new_selection, true);
    }

    fn select_object_list_ex(
        &mut self,
        player_mask: u32,
        object_ids: Vec<ObjectId>,
        create_new_selection: bool,
        play_voice: bool,
    ) {
        if player_mask == 0 {
            return;
        }
        let mut player_ids: Vec<u32> = self
            .players
            .keys()
            .copied()
            .filter(|&id| (player_mask & (1u32 << id.min(31))) != 0)
            .collect();
        if player_ids.is_empty() {
            return;
        }
        player_ids.sort_unstable();

        let mut accepted: Vec<ObjectId> = Vec::new();
        for &object_id in &object_ids {
            if accepted.contains(&object_id) {
                continue;
            }
            let Some(obj) = self.objects.get(&object_id) else {
                continue;
            };
            if !obj.is_selectable() {
                continue;
            }
            // C++ GameLogic.cpp:2602-2606 — mass-selectable gate only on add.
            if !create_new_selection && !host_object_is_mass_selectable(obj) {
                continue;
            }
            accepted.push(object_id);
        }

        if create_new_selection {
            let mut previous: Vec<ObjectId> = Vec::new();
            for &pid in &player_ids {
                if let Some(player) = self.players.get(&pid) {
                    for &old_id in &player.selected_objects {
                        if !previous.contains(&old_id) {
                            previous.push(old_id);
                        }
                    }
                }
            }
            for &old_id in &previous {
                if accepted.contains(&old_id) {
                    continue;
                }
                if let Some(obj) = self.objects.get_mut(&old_id) {
                    obj.deselect();
                }
            }
        }

        for &object_id in &accepted {
            if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.select();
                // C++ Drawable::flashAsSelected residual on select / create-team.
                obj.flash_as_selected();
            }
            // C++ Drawable::onSelected → contain->clientVisibleContainedFlashAsSelected.
            self.client_visible_contained_flash_as_selected(object_id);
        }

        let any_local = player_ids
            .iter()
            .any(|&id| self.players.get(&id).map(|p| p.is_local).unwrap_or(false));

        for &pid in &player_ids {
            if let Some(player) = self.players.get_mut(&pid) {
                if create_new_selection {
                    player.selected_objects = accepted.clone();
                } else {
                    for &id in &accepted {
                        if !player.selected_objects.contains(&id) {
                            player.selected_objects.push(id);
                        }
                    }
                }
            }
        }

        // C++ VoiceSelect from ThingTemplate INI (not `{template}VoiceSelect` / UnitVoiceSelect).
        // CommandXlat.cpp:3502-3516 weeds to isLocallyControlled before pickAndPlay.
        if play_voice && any_local {
            self.queue_create_selected_group_voice(&accepted);
        }

        log::debug!(
            "mask {:#x} selected {} objects (create_new={})",
            player_mask,
            accepted.len(),
            create_new_selection
        );
    }

    /// C++ `TheInGameUI->deselectDrawable` for one host object.
    pub fn deselect_drawable(&mut self, id: ObjectId) {
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.deselect();
        }
        for player in self.players.values_mut() {
            player.selected_objects.retain(|&x| x != id);
        }
    }

    /// Drain leftover `updateHiddenStatus` onto live player selection.
    pub fn drain_hidden_drawable_selection(&mut self) {
        let hidden: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, obj)| obj.drawable_is_effectively_hidden())
            .map(|(&id, _)| id)
            .collect();
        for id in hidden {
            self.deselect_drawable(id);
        }
    }

    /// Drain leftover `maskObject` deselect onto live player selection.
    pub fn drain_masked_object_selection(&mut self) {
        for id in crate::game_logic::object::drain_mask_deselects() {
            self.deselect_drawable(id);
        }
    }

    /// C++ `Drawable::onSelected` → `contain->clientVisibleContainedFlashAsSelected`.
    /// `OpenContain` default is empty. Overlord walks contained
    /// `KINDOF_PORTABLE_STRUCTURE`; Helix flashes `getPortableStructure()`.
    pub(crate) fn client_visible_contained_flash_as_selected(&mut self, host_id: ObjectId) {
        let Some(host) = self.objects.get(&host_id) else {
            return;
        };
        let mut candidates: Vec<ObjectId> = Vec::new();
        if let Some(id) = host.overlord_portable_occupant {
            candidates.push(id);
        }
        for &id in &host.occupants {
            if !candidates.contains(&id) {
                candidates.push(id);
            }
        }
        let flash: Vec<ObjectId> = candidates
            .into_iter()
            .filter(|&id| {
                self.objects.get(&id).is_some_and(|occ| {
                    crate::game_logic::host_battlemaster::is_portable_structure_template(
                        &occ.template_name,
                    )
                })
            })
            .collect();
        for id in flash {
            if let Some(occ) = self.objects.get_mut(&id) {
                occ.flash_as_selected();
            }
        }
    }

    /// C++ CommandXlat.cpp:3490-3516 `MSG_CREATE_SELECTED_GROUP` / `SELECT_TEAM`
    /// pickAndPlay after weeding to `isLocallyControlled()`.
    pub fn queue_create_selected_group_voice(&mut self, unit_ids: &[ObjectId]) {
        let voice_ids: Vec<ObjectId> = unit_ids
            .iter()
            .copied()
            .filter(|&id| self.is_object_locally_controlled(id))
            .collect();
        if voice_ids.is_empty() {
            return;
        }
        self.queue_picked_unit_voice(
            &voice_ids,
            crate::game_logic::audio_dispatch_impl::UnitVoiceSlot::Select,
        );
    }

    /// Issue move command to selected objects (with pathfinding)
    pub fn command_move(&mut self, player_id: u32, target_position: Vec3) {
        if let Some(player) = self.players.get(&player_id) {
            let selected: Vec<ObjectId> = player
                .selected_objects
                .iter()
                .copied()
                .filter(|object_id| {
                    self.objects
                        .get(object_id)
                        .is_some_and(|obj| obj.owner_player_id == Some(player_id))
                })
                .collect();
            for &object_id in &selected {
                let is_mobile = self
                    .objects
                    .get(&object_id)
                    .map(|obj| obj.is_mobile())
                    .unwrap_or(false);
                if !is_mobile {
                    continue;
                }

                // Host pathfinding / move channel (default production path).
                self.move_object_with_pathfinding(object_id, target_position, None);
            }
            // C++ VoiceMove from ThingTemplate INI (not `{template}VoiceMove` / UnitVoiceMove).
            let local = self
                .players
                .get(&player_id)
                .map(|p| p.is_local)
                .unwrap_or(false);
            if local {
                self.queue_picked_move_voice(&selected);
            }
            log::trace!(
                "{} commanded {} units to move to {:?}",
                player_id,
                selected.len(),
                target_position
            );
        }
    }

    /// Wave 930: single direct-order authority boundary.
    #[inline]
    pub fn apply_direct_player_order(&mut self, order: DirectPlayerOrder) {
        match order {
            DirectPlayerOrder::Attack { player_id, target } => {
                self.command_attack(player_id, target);
            }
            DirectPlayerOrder::Stop { player_id } => {
                self.command_stop(player_id);
            }
            DirectPlayerOrder::Move { player_id, dest } => {
                self.command_move(player_id, dest);
            }
            DirectPlayerOrder::AttackMove { player_id, dest } => {
                self.command_attack_move(player_id, dest);
            }
        }
    }

    /// Wave 931: single object-lifecycle authority boundary.
    #[inline]
    pub fn apply_object_lifecycle_op(&mut self, op: ObjectLifecycleOp) -> ObjectLifecycleResult {
        match op {
            ObjectLifecycleOp::Create {
                name,
                team,
                spawn_at,
            } => ObjectLifecycleResult::Created(self.create_object(&name, team, spawn_at)),
            ObjectLifecycleOp::Destroy { id } => {
                self.destroy_object(id);
                ObjectLifecycleResult::Destroyed
            }
            ObjectLifecycleOp::ForceCompleteConstruction { id } => {
                ObjectLifecycleResult::Bool(self.force_complete_construction(id))
            }
            ObjectLifecycleOp::ClearMovementPath { id } => {
                ObjectLifecycleResult::Bool(self.clear_unit_movement_path(id))
            }
            ObjectLifecycleOp::AdjustGuardRadius { id, delta } => {
                ObjectLifecycleResult::Radius(self.adjust_unit_guard_radius(id, delta))
            }
            ObjectLifecycleOp::EnqueueProduction {
                producer,
                template_name,
            } => ObjectLifecycleResult::Bool(self.enqueue_production(producer, template_name)),
            ObjectLifecycleOp::CancelProduction { id, template_name } => {
                ObjectLifecycleResult::Bool(self.cancel_production(id, template_name))
            }
            ObjectLifecycleOp::CancelProductionAtIndex { id, queue_index } => {
                ObjectLifecycleResult::Bool(self.cancel_production_at_index(id, queue_index))
            }
        }
    }

    /// Wave 932: single command-pipeline authority boundary.
    #[inline]
    pub fn apply_command_pipeline_op(&mut self, op: CommandPipelineOp) -> bool {
        match op {
            CommandPipelineOp::Queue { command } => {
                self.queue_command(command);
                false
            }
            CommandPipelineOp::QueueAndProcess { command } => {
                self.queue_and_process_command(command)
            }
            CommandPipelineOp::ProcessIfNeeded => self.process_commands_if_needed(),
        }
    }

    /// Wave 933: single session-control authority boundary.
    #[inline]
    pub fn apply_session_control_op(&mut self, op: SessionControlOp) {
        match op {
            SessionControlOp::SelectObjects { player_id, ids } => {
                self.select_objects(player_id, ids);
            }
            SessionControlOp::SetPaused { paused } => {
                self.set_paused(paused);
            }
            SessionControlOp::SetCameraFollow { id } => {
                self.set_camera_follow_object(id);
            }
            SessionControlOp::StartNewGameWithFaction {
                mode,
                player_id,
                faction_team,
                setup_skirmish_ai,
            } => {
                self.start_new_game_with_faction(mode, player_id, faction_team, setup_skirmish_ai);
            }
            SessionControlOp::StartNewGameWithPlayerTemplate {
                mode,
                player_id,
                player_template,
            } => {
                self.start_new_game_with_player_template(mode, player_id, player_template);
            }
            SessionControlOp::Reset => {
                self.reset();
            }
            SessionControlOp::OverrideWorldSize { width, height } => {
                self.override_world_size(width, height);
            }
        }
    }

    /// Wave 934: single host-support residual authority boundary.
    #[inline]
    pub fn apply_host_support_op(&mut self, op: HostSupportOp) -> HostSupportResult {
        match op {
            HostSupportOp::EnsureBarracksBuildingData { id } => {
                HostSupportResult::Bool(self.ensure_barracks_building_data(id))
            }
            HostSupportOp::ForceEnsureBarracksBuildingData { id } => {
                HostSupportResult::Bool(self.force_ensure_barracks_building_data(id))
            }
            HostSupportOp::EnsurePlayerMinSupplies { player_id, floor } => {
                self.ensure_player_min_supplies(player_id, floor);
                HostSupportResult::Unit
            }
            HostSupportOp::UpdateShellWithBudget { dt, budget } => {
                HostSupportResult::Snapshot(self.update_shell_with_budget(dt, budget))
            }
            HostSupportOp::ProcessDestroyListIfNeeded => {
                self.process_destroy_list_if_needed();
                HostSupportResult::Unit
            }
            HostSupportOp::InsertThingTemplate { name, template } => {
                self.templates.insert(name, template);
                HostSupportResult::Unit
            }
        }
    }

    /// Wave 937: single production complete/spawn authority boundary.
    #[inline]
    pub fn apply_production_authority_op(
        &mut self,
        op: ProductionAuthorityOp,
    ) -> ProductionAuthorityResult {
        match op {
            ProductionAuthorityOp::ApplyCompletionsAfterReadyWriteback { dt } => {
                self.host_apply_production_completions_after_ready_writeback(dt);
                ProductionAuthorityResult::Unit
            }
            ProductionAuthorityOp::SpawnUnit {
                template,
                team,
                owner_player_id,
                spawn_pos,
            } => ProductionAuthorityResult::Spawned(match owner_player_id {
                Some(player_id) => {
                    self.host_spawn_production_unit_for_player(&template, player_id, spawn_pos)
                }
                None => self.host_spawn_production_unit(&template, team, spawn_pos),
            }),
            ProductionAuthorityOp::ApplySpawnReadyCompletions => {
                self.host_apply_production_spawn_ready_completions();
                ProductionAuthorityResult::Unit
            }
            ProductionAuthorityOp::ApplyDoorReadyCompletions => {
                self.host_apply_production_door_ready_completions();
                ProductionAuthorityResult::Unit
            }
        }
    }

    /// Wave 938: single post-writeback complete authority boundary.
    #[inline]
    pub fn apply_post_writeback_complete_op(&mut self, op: PostWritebackCompleteOp) {
        match op {
            PostWritebackCompleteOp::ConstructionCompletionsAfterReadyWriteback => {
                self.host_apply_construction_completions_after_ready_writeback();
            }
            PostWritebackCompleteOp::SellCompletionsAfterReadyWriteback => {
                self.host_apply_sell_completions_after_ready_writeback();
            }
            PostWritebackCompleteOp::SpecialPowerReadyAfterWriteback => {
                self.host_apply_special_power_ready_after_writeback();
            }
        }
    }

    /// Wave 939: single ready-log drain authority boundary (shadow post-writeback).
    #[inline]
    pub fn apply_ready_log_drain_op(&mut self, op: ReadyLogDrainOp) -> usize {
        match op {
            ReadyLogDrainOp::Contain => self.host_apply_contain_ready_completions(),
            ReadyLogDrainOp::Projectiles => self.host_apply_projectiles_ready_completions(),
            ReadyLogDrainOp::AttackTarget => self.host_apply_attack_target_ready_completions(),
            ReadyLogDrainOp::AiState => self.host_apply_ai_state_ready_completions(),
            ReadyLogDrainOp::Movement => self.host_apply_movement_ready_completions(),
            ReadyLogDrainOp::FireIntent => self.host_apply_fire_intent_ready_completions(),
            ReadyLogDrainOp::MoveTarget => self.host_apply_move_target_ready_completions(),
            ReadyLogDrainOp::Transform => self.host_apply_transform_ready_completions(),
            ReadyLogDrainOp::Locomotor => self.host_apply_locomotor_ready_completions(),
            ReadyLogDrainOp::AiRequest => self.host_apply_ai_request_ready_completions(),
            ReadyLogDrainOp::Hijacker => self.host_apply_hijacker_ready_completions(),
            ReadyLogDrainOp::PhysicsMotive => self.host_apply_physics_motive_ready_completions(),
            ReadyLogDrainOp::BounceLand => self.host_apply_bounce_land_ready_completions(),
            ReadyLogDrainOp::CombatStatus => self.host_apply_combat_status_ready_completions(),
            ReadyLogDrainOp::BodyDamage => self.host_apply_body_damage_ready_completions(),
            ReadyLogDrainOp::DeathType => self.host_apply_death_type_ready_completions(),
            ReadyLogDrainOp::RadarExtend => self.host_apply_radar_extend_ready_completions(),
            ReadyLogDrainOp::ShockStun => self.host_apply_shock_stun_ready_completions(),
            ReadyLogDrainOp::ConstructionCompleteClear => {
                self.host_apply_construction_complete_clear_ready_completions()
            }
            ReadyLogDrainOp::SoleHealing => self.host_apply_sole_healing_ready_completions(),
            ReadyLogDrainOp::AiMood => self.host_apply_ai_mood_ready_completions(),
            ReadyLogDrainOp::Owner => self.host_apply_owner_ready_completions(),
            ReadyLogDrainOp::Veterancy => self.host_apply_veterancy_ready_completions(),
            ReadyLogDrainOp::WeaponBonus => self.host_apply_weapon_bonus_ready_completions(),
            ReadyLogDrainOp::FaerieFire => self.host_apply_faerie_fire_ready_completions(),
            ReadyLogDrainOp::Repulsor => self.host_apply_repulsor_ready_completions(),
            ReadyLogDrainOp::DisableTimers => self.host_apply_disable_timers_ready_completions(),
            ReadyLogDrainOp::WeaponSlot => self.host_apply_weapon_slot_ready_completions(),
            ReadyLogDrainOp::EntityPower => self.host_apply_entity_power_ready_completions(),
            ReadyLogDrainOp::Turret => self.host_apply_turret_ready_completions(),
            ReadyLogDrainOp::StealthDelay => self.host_apply_stealth_delay_ready_completions(),
            ReadyLogDrainOp::CombatAttack => self.host_apply_combat_attack_ready_completions(),
            ReadyLogDrainOp::TargetLocation => self.host_apply_target_location_ready_completions(),
            ReadyLogDrainOp::Detector => self.host_apply_detector_ready_completions(),
            ReadyLogDrainOp::ContinuousFire => self.host_apply_continuous_fire_ready_completions(),
            ReadyLogDrainOp::Guard => self.host_apply_guard_ready_completions(),
            ReadyLogDrainOp::AiAttitude => self.host_apply_ai_attitude_ready_completions(),
            ReadyLogDrainOp::WeaponSet => self.host_apply_weapon_set_ready_completions(),
            ReadyLogDrainOp::Overcharge => self.host_apply_overcharge_ready_completions(),
            ReadyLogDrainOp::Hive => self.host_apply_hive_ready_completions(),
            ReadyLogDrainOp::StealthFlags => self.host_apply_stealth_flags_ready_completions(),
            ReadyLogDrainOp::Overlord => self.host_apply_overlord_ready_completions(),
            ReadyLogDrainOp::CommandSet => self.host_apply_command_set_ready_completions(),
            ReadyLogDrainOp::Disguise => self.host_apply_disguise_ready_completions(),
            ReadyLogDrainOp::VisionCamo => self.host_apply_vision_camo_ready_completions(),
            ReadyLogDrainOp::WeaponStats => self.host_apply_weapon_stats_ready_completions(),
            ReadyLogDrainOp::SelectionRadius => {
                self.host_apply_selection_radius_ready_completions()
            }
            ReadyLogDrainOp::ModelCondition => self.host_apply_model_condition_ready_completions(),
            ReadyLogDrainOp::DemoMineCheer => self.host_apply_demo_mine_cheer_ready_completions(),
            ReadyLogDrainOp::CrushVision => self.host_apply_crush_vision_ready_completions(),
            ReadyLogDrainOp::BuildingType => self.host_apply_building_type_ready_completions(),
            ReadyLogDrainOp::Identity => self.host_apply_identity_ready_completions(),
            ReadyLogDrainOp::GroundHeight => self.host_apply_ground_height_ready_completions(),
            ReadyLogDrainOp::Economy => self.host_apply_economy_ready_completions(),
            ReadyLogDrainOp::Upgrade => self.host_apply_upgrade_ready_completions(),
            ReadyLogDrainOp::StoredSupplies => self.host_apply_stored_supplies_ready_completions(),
        }
    }

    /// Wave 940: batch post-writeback sole-tick residuals (single authority boundary).
    #[inline]
    pub fn apply_post_writeback_sole_ticks(&mut self) {
        // Order matches shadow_session_after_host_tick (Waves 823–827).
        self.tick_patriot_assist_lasers_sole();
        self.tick_pending_patriot_assists_sole();
        self.tick_zone_damage_fields_sole();
        self.tick_combat_field_residuals_sole();
        self.tick_host_systems_residuals_sole();
    }

    /// Wave 940: host ObjectId create/mark-destroy authority boundary.
    #[inline]
    pub fn apply_host_object_id_op(&mut self, op: HostObjectIdOp) -> HostObjectIdResult {
        match op {
            HostObjectIdOp::MarkForDestruction { id, team } => {
                self.mark_object_for_destruction(id, team);
                HostObjectIdResult::Unit
            }
            HostObjectIdOp::Create {
                template,
                team,
                spawn_at,
            } => HostObjectIdResult::Created(self.create_object(&template, team, spawn_at)),
        }
    }

    /// Wave 941/942: host residual mutation authority boundary.
    #[inline]
    pub fn apply_host_residual_mutation_op(&mut self, op: HostResidualMutationOp) {
        match op {
            HostResidualMutationOp::PoisonDot {
                object,
                amount,
                death_type,
            } => {
                if let Some(obj) = self.host_objects_mut().get_mut(&object) {
                    let _ = obj.take_damage_from_typed_death_fx(
                        amount,
                        None,
                        crate::game_logic::combat::DamageType::Unresistable,
                        death_type,
                        Some(crate::game_logic::host_poisoned_behavior::poison_dot_fx_override()),
                    );
                }
            }
            HostResidualMutationOp::ForceKill {
                id,
                death_type,
                refresh_model_condition,
                mark_destroy,
            } => {
                if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                    obj.health.current = 0.0;
                    obj.status.destroyed = true;
                    if let Some(dt) = death_type {
                        obj.status.death_type = dt;
                    }
                    if refresh_model_condition {
                        obj.refresh_model_condition_bits();
                    }
                }
                if mark_destroy {
                    self.mark_object_for_destruction(id, None);
                }
            }
            HostResidualMutationOp::SetPendingFireWhenDamaged {
                id,
                weapon,
                overwrite,
            } => {
                if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                    if overwrite || obj.pending_fire_when_damaged_weapon.is_none() {
                        obj.pending_fire_when_damaged_weapon = Some(weapon);
                    }
                }
            }
            HostResidualMutationOp::LethalExpire {
                id,
                position,
                effectively_dead,
                clear,
                mark_destroy_team,
            } => {
                if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                    if let Some(pos) = position {
                        obj.set_position(pos);
                    }
                    if crate::gameworld_shadow::gameworld_damage_authority_live() {
                        let hp = obj.health.current.max(1.0);
                        let oid = obj.id;
                        crate::game_logic::host_damage_log::record(oid, hp, None, true);
                    } else {
                        obj.health.current = 0.0;
                    }
                    obj.status.destroyed = true;
                    if effectively_dead {
                        obj.status.effectively_dead = true;
                    }
                    match clear {
                        ObjectIdentityClear::None => {}
                        ObjectIdentityClear::FlashbangGrenadeProjectile => {
                            obj.flashbang_grenade_projectile = false;
                        }
                        ObjectIdentityClear::ScorpionMissileProjectile => {
                            obj.scorpion_missile_projectile = false;
                        }
                        ObjectIdentityClear::SpySatellitePing => {
                            obj.spy_satellite_ping = false;
                        }
                        ObjectIdentityClear::AngryMobMember => {
                            obj.angry_mob_member = false;
                        }
                        ObjectIdentityClear::AuroraBombProjectile => {
                            obj.aurora_bomb_projectile = false;
                        }
                        ObjectIdentityClear::InfernoShellProjectile => {
                            obj.inferno_shell_projectile = false;
                        }
                        ObjectIdentityClear::ToxinStreamProjectile => {
                            obj.toxin_stream_projectile = false;
                        }
                        ObjectIdentityClear::AngryMobProjectile => {
                            obj.angry_mob_projectile = false;
                        }
                        ObjectIdentityClear::CannonShellProjectile => {
                            obj.scud_launcher_missile_projectile = false;
                            obj.neutron_cannon_shell_projectile = false;
                            obj.nuke_cannon_shell_projectile = false;
                        }
                        ObjectIdentityClear::LeafletContainer => {
                            obj.leaflet_container = false;
                        }
                        ObjectIdentityClear::ParadropCargo => {
                            obj.paradrop_parachute = false;
                        }
                        ObjectIdentityClear::ComancheRocketPodProjectile => {
                            obj.comanche_rocket_pod_projectile = false;
                        }
                        ObjectIdentityClear::EmpPulseSpheroid => {
                            obj.emp_pulse_spheroid = false;
                        }
                        ObjectIdentityClear::FieldObject(kind) => {
                            use crate::game_logic::host_field_object_expire_log::FieldObjectKind;
                            match kind {
                                FieldObjectKind::NukeRadiation => {
                                    obj.nuke_radiation_field = false;
                                }
                                FieldObjectKind::AnthraxToxin => {
                                    obj.anthrax_toxin_field = false;
                                }
                                FieldObjectKind::InfernoFire => {
                                    obj.inferno_fire_field = false;
                                }
                                FieldObjectKind::SpectreHowitzerShell => {
                                    obj.spectre_howitzer_shell = false;
                                }
                                FieldObjectKind::CountermeasureFlare => {
                                    obj.countermeasure_flare = false;
                                }
                                FieldObjectKind::PointDefenseLaserBeam => {
                                    obj.point_defense_laser_beam = false;
                                }
                                FieldObjectKind::WeaponLaserBeam => {
                                    obj.weapon_laser_beam = false;
                                }
                                FieldObjectKind::ParticleTrailRemnant => {
                                    obj.particle_trail_remnant = false;
                                }
                                FieldObjectKind::ParticleOrbitalLaser => {
                                    obj.particle_orbital_laser = false;
                                }
                                FieldObjectKind::ParticleConnectorLaser => {
                                    obj.particle_connector_laser = false;
                                }
                                FieldObjectKind::FirewallSegment => {
                                    obj.firewall_segment = false;
                                    obj.firewall_segment_wall_id = None;
                                    obj.firewall_segment_dir = None;
                                }
                                FieldObjectKind::RadarVanPing => {
                                    obj.radar_van_ping = false;
                                }
                                FieldObjectKind::MoneyCrate => {}
                            }
                        }
                    }
                }
                if let Some(team) = mark_destroy_team {
                    self.apply_host_object_id_op(HostObjectIdOp::MarkForDestruction { id, team });
                }
            }
            HostResidualMutationOp::DestroyBomb { id, mark_destroy } => {
                if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                    obj.health.current = 0.0;
                    obj.status.destroyed = true;
                }
                if mark_destroy {
                    self.mark_object_for_destruction(id, None);
                }
            }
            HostResidualMutationOp::SetModelConditionBits {
                id,
                bits,
                count_update,
            } => {
                if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                    let before = obj.model_condition_bits;
                    obj.model_condition_bits = bits;
                    if count_update && obj.model_condition_bits != before {
                        self.actively_constructing_updates =
                            self.actively_constructing_updates.saturating_add(1);
                    }
                }
            }
            HostResidualMutationOp::PowerPlantRodsComplete {
                id,
                model_condition_bits,
            } => {
                if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                    obj.model_condition_bits = model_condition_bits;
                    obj.power_plant_rods_done_frame = 0;
                    obj.power_plant_rods_extended = true;
                }
                self.special_power_completion_log.record_rods_complete();
            }
            HostResidualMutationOp::SetWeaponBonusHorde {
                id,
                now_horde,
                was_horde,
                grant,
            } => {
                if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                    let was = obj.weapon_bonus_horde;
                    obj.weapon_bonus_horde = now_horde;
                    obj.record_host_weapon_bonus();
                    obj.apply_horde_terrain_decal(
                        was,
                        now_horde,
                        crate::game_logic::host_battlemaster::leftover_horde_draw_icon_ui(),
                    );

                    if now_horde && !was {
                        match grant {
                            HordeGrantCounter::Battlemaster => {
                                self.battlemaster_residual_horde_grants =
                                    self.battlemaster_residual_horde_grants.saturating_add(1);
                            }
                            HordeGrantCounter::RedGuard => {
                                self.red_guard_residual_horde_grants =
                                    self.red_guard_residual_horde_grants.saturating_add(1);
                            }
                            HordeGrantCounter::TankHunter => {
                                self.tank_hunter_residual_horde_grants =
                                    self.tank_hunter_residual_horde_grants.saturating_add(1);
                            }
                            HordeGrantCounter::Minigunner => {
                                self.minigunner_residual_horde_grants =
                                    self.minigunner_residual_horde_grants.saturating_add(1);
                            }
                            HordeGrantCounter::None => {}
                        }
                    }
                }

                if now_horde != was_horde || now_horde {
                    match grant {
                        HordeGrantCounter::Battlemaster => {
                            self.refresh_battlemaster_weapon(id);
                        }
                        HordeGrantCounter::RedGuard => {
                            self.refresh_red_guard_weapon(id);
                        }
                        HordeGrantCounter::TankHunter => {
                            self.refresh_tank_hunter_weapon(id);
                        }
                        HordeGrantCounter::Minigunner => {
                            self.refresh_minigunner_weapon(id);
                        }
                        HordeGrantCounter::None => {}
                    }
                    // C++ evaluateMoraleBonus on every HordeUpdate unit.
                    // Battlemaster refresh is a no-op for Dragon/Inferno/
                    // Gattling/Overlord; this still stamps NATIONALISM.
                    self.evaluate_horde_morale_bonus(id);
                }
            }
            HostResidualMutationOp::ApplyStingerHiveState {
                id,
                hive_slave_count,
                hive_slave_hp,
                hive_slave_respawn_frame,
                slaves_alive,
                slaves_hp,
            } => {
                if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                    obj.hive_slave_count = hive_slave_count;
                    obj.hive_slave_hp = hive_slave_hp;
                    obj.hive_slave_respawn_frame = hive_slave_respawn_frame;
                    for i in 0..3 {
                        obj.hive_slaves[i].alive = slaves_alive[i];
                        obj.hive_slaves[i].hp = slaves_hp[i];
                    }
                    obj.record_host_hive();
                }
                self.stinger_hive_residual_respawns =
                    self.stinger_hive_residual_respawns.saturating_add(1);
            }
            HostResidualMutationOp::SetPosition {
                id,
                position,
                sticky_follow_tick,
            } => {
                if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                    obj.set_position(position);
                }
                if sticky_follow_tick {
                    self.sticky_bomb_follow_ticks = self.sticky_bomb_follow_ticks.saturating_add(1);
                }
            }
            HostResidualMutationOp::ConfigureSpawnedPayload {
                id,
                producer,
                target,
                kind,
            } => {
                if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                    obj.producer_id = Some(producer);
                    let parachuting = matches!(kind, SpawnedPayloadKind::ParadropParachute);
                    match kind {
                        SpawnedPayloadKind::DaisyCutter { moab_template } => {
                            obj.daisy_cutter_bomb = true;
                            if let Some(name) = moab_template {
                                obj.template_name = name;
                            }
                            obj.movement.velocity = glam::Vec3::new(0.0, -16.0, 0.0);
                        }
                        SpawnedPayloadKind::AnthraxBomb => {
                            obj.anthrax_bomb_payload = true;
                            obj.movement.velocity = glam::Vec3::new(0.0, -14.0, 0.0);
                        }
                        SpawnedPayloadKind::ClusterMinesBomb => {
                            obj.cluster_mines_bomb = true;
                            obj.movement.velocity = glam::Vec3::new(0.0, -14.0, 0.0);
                        }
                        SpawnedPayloadKind::EmpPulseBomb => {
                            obj.emp_pulse_bomb = true;
                            obj.movement.velocity = glam::Vec3::new(0.0, -14.0, 0.0);
                        }
                        SpawnedPayloadKind::A10StrikeMissile => {
                            obj.a10_strike_missile = true;
                            obj.movement.velocity =
                                crate::game_logic::host_a10_strike_drop_log::a10_missile_fire_velocity(
                                    obj.get_position(),
                                    target,
                                    glam::Vec3::ZERO,
                                );
                        }
                        SpawnedPayloadKind::ArtilleryBarrageShell => {
                            obj.artillery_barrage_shell = true;
                            obj.movement.velocity = glam::Vec3::new(0.0, -18.0, 0.0);
                        }
                        SpawnedPayloadKind::CarpetBomb => {
                            obj.carpet_bomb_payload = true;
                            obj.movement.velocity = glam::Vec3::new(0.0, -15.0, 0.0);
                        }
                        SpawnedPayloadKind::LeafletContainer => {
                            obj.leaflet_container = true;
                            obj.movement.velocity = glam::Vec3::new(0.0, -12.0, 0.0);
                        }
                        SpawnedPayloadKind::ParadropParachute => {
                            obj.paradrop_parachute = true;
                            obj.movement.velocity = glam::Vec3::new(0.0, -8.0, 0.0);
                        }
                    }
                    let _ = obj.set_smart_bomb_target(target);
                    if parachuting {
                        let _ = obj.apply_eject_parachuting();
                    }
                }
            }
            HostResidualMutationOp::ApplyRawHpDamage { id, amount } => {
                // Wave 943: host-only damage fallback (no shadow entity).
                if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                    if obj.status.destroyed {
                        // already dead
                    } else {
                        obj.health.damage(amount);
                        if !obj.health.is_alive() {
                            obj.status.destroyed = true;
                            obj.set_ai_state(crate::game_logic::AIState::Idle);
                            obj.target = None;
                        }
                        obj.refresh_model_condition_bits();
                    }
                }
            }
        }
    }

    /// Wave 943: apply post-armor damage for host objects with no shadow mapping.
    /// Returns number of host objects mutated.
    pub fn apply_host_unmapped_damage_fallback(
        &mut self,
        events: &[crate::game_logic::host_damage_log::HostDamageEvent],
        mut shadow_mapped: impl FnMut(ObjectId) -> bool,
    ) -> usize {
        let mut fallback = 0usize;
        for ev in events {
            if shadow_mapped(ev.target) {
                continue;
            }
            let eligible = self
                .host_objects()
                .get(&ev.target)
                .is_some_and(|o| !o.status.destroyed);
            if !eligible {
                continue;
            }
            self.apply_host_residual_mutation_op(HostResidualMutationOp::ApplyRawHpDamage {
                id: ev.target,
                amount: ev.amount,
            });
            fallback += 1;
        }
        fallback
    }

    /// Wave 944: apply one shadow→host writeback mutation.
    pub fn apply_host_writeback_op(&mut self, op: HostWritebackOp) -> bool {
        match op {
            HostWritebackOp::Health {
                id,
                current,
                maximum,
                destroy,
            } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                let max = maximum.max(1.0);
                obj.health.current = current.min(max);
                obj.max_health = max;
                obj.health.maximum = max;
                if destroy {
                    obj.status.destroyed = true;
                    obj.ai_state = crate::game_logic::AIState::Idle;
                    obj.target = None;
                }
                true
            }
            HostWritebackOp::Experience { id, points, level } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                if let Some(pts) = points {
                    obj.experience.current = pts;
                }
                if let Some(lvl) = level {
                    obj.experience.level = lvl;
                }
                true
            }
            HostWritebackOp::Transform {
                id,
                position,
                orientation,
            } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.set_position(position);
                obj.set_orientation(orientation);
                true
            }
            HostWritebackOp::AttackTarget {
                id,
                target,
                clear_target_location,
            } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.target = target;
                if clear_target_location {
                    obj.target_location = None;
                }
                true
            }
            HostWritebackOp::MoveTarget { id, destination } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.movement.target_position = destination;
                true
            }
            HostWritebackOp::AiState { id, ordinal } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.ai_state =
                    crate::gameworld_shadow::GameWorldShadow::ai_state_from_ordinal(ordinal);
                true
            }
            HostWritebackOp::AiAttitude { id, attitude } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.ai_attitude = attitude.clamp(-2, 2);
                true
            }
            HostWritebackOp::Owner {
                id,
                team,
                team_color,
                owner_player_id,
            } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.team = team;
                obj.team_color = team_color;
                obj.owner_player_id = owner_player_id;
                true
            }
            HostWritebackOp::SpecialPower {
                id,
                ready,
                cooldown_remaining,
                cooldown,
            } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.special_power_ready = ready;
                obj.special_power_cooldown_remaining = cooldown_remaining.max(0.0);
                obj.special_power_cooldown = cooldown.max(0.0);
                true
            }
            HostWritebackOp::Overcharge { id, enabled } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.overcharge_enabled = enabled;
                true
            }
            HostWritebackOp::WeaponSlot { id, slot } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.active_weapon_slot = slot;
                true
            }
            HostWritebackOp::SelectionRadius { id, radius } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.selection_radius = radius;
                true
            }
            HostWritebackOp::EntityPower {
                id,
                provided,
                consumed,
            } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.power_provided = provided;
                obj.power_consumed = consumed;
                true
            }
            HostWritebackOp::TargetLocation { id, location } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.target_location = location;
                true
            }
            HostWritebackOp::CommandSet { id, override_name } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.command_set_override = override_name;
                true
            }
            HostWritebackOp::GroundHeight {
                id,
                height,
                from_terrain,
            } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.ground_height = height;
                obj.ground_height_from_terrain = from_terrain;
                true
            }
            HostWritebackOp::BodyDamage { id, state } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.body_damage_state = state;
                true
            }
            HostWritebackOp::DeathType { id, death_type } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.status.death_type = death_type;
                true
            }
            HostWritebackOp::StoredSupplies { id, supplies } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.stored_resources.supplies = supplies;
                true
            }
            HostWritebackOp::FaerieFire {
                id,
                active,
                until_frame,
            } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.status.faerie_fire = active;
                obj.faerie_fire_until_frame = if active { until_frame } else { 0 };
                true
            }
            HostWritebackOp::Repulsor {
                id,
                active,
                until_frame,
            } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.repulsor_until_frame = until_frame;
                obj.status.repulsor = active;
                true
            }
            HostWritebackOp::Detector {
                id,
                is_detector,
                range,
                rate_frames,
            } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.is_detector = is_detector;
                obj.detection_range = range.max(0.0);
                obj.detection_rate_frames = rate_frames;
                true
            }
            HostWritebackOp::Guard {
                id,
                position,
                target,
                radius,
            } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.guard_position = position;
                obj.guard_target = target;
                obj.guard_radius = radius;
                true
            }
        }
    }

    /// Wave 946: scoped host-object mutation authority.
    /// Shadow writebacks mutate host objects only through this boundary
    /// (no direct `get_objects_mut` dual-writes from the shadow crate).
    ///
    /// When a GameWorld shadow session is coupled the HashMap is a read-view:
    /// overlay from GameWorld, apply residual side effects, do not last-write
    /// HP/pose/target/weapon/contain back. Shadow-off keeps the host field.
    pub fn with_host_object_mut<R>(
        &mut self,
        id: ObjectId,
        f: impl FnOnce(&mut crate::game_logic::object::Object) -> R,
    ) -> Option<R> {
        let obj = self.host_object_mut(id)?;
        let r = f(obj);
        if !crate::gameworld_shadow::shadow_coupled_tick_active() {
            self.push_coupled_host_object_mutations(id);
        }
        self.host_view_dirty.remove(&id);
        Some(r)
    }

    /// Copy GameWorld HP / pose / target / fat fields onto the HashMap view.
    fn overlay_object_from_gameworld(&mut self, id: ObjectId) {
        use crate::gameworld_shadow::{
            coupled_entity_fat_view, coupled_entity_health, coupled_entity_pose,
            coupled_entity_target_host, with_active_shadow,
        };
        let mapped = with_active_shadow(|s| s.entity_for_host(id).is_some()).unwrap_or(false);
        if !mapped {
            return;
        }
        let hp = coupled_entity_health(id);
        let pose = coupled_entity_pose(id);
        let target = coupled_entity_target_host(id);
        let fat = coupled_entity_fat_view(id);
        let skip_hp = crate::game_logic::host_damage_log::has_pending(id);
        let skip_weapon = crate::game_logic::host_weapon_stats_log::has_pending(id);
        let skip_ai = crate::game_logic::host_ai_state_log::has_pending(id)
            || crate::game_logic::host_combat_attack_log::has_pending(id);
        let skip_contain = crate::game_logic::host_contain_log::has_pending(id);
        let skip_move = crate::game_logic::host_movement_log::has_pending(id);
        let Some(obj) = self.objects.get_mut(&id) else {
            return;
        };
        if !skip_hp {
            if let Some(h) = hp {
                obj.health.current = h;
                if h <= 0.0 {
                    obj.status.destroyed = true;
                }
            }
        }
        if let Some([x, y, z]) = pose {
            obj.set_position(glam::Vec3::new(x, y, z));
        }
        obj.target = target;
        if let Some(fat) = fat {
            if !skip_weapon {
                if let Some(w) = obj.weapon.as_mut() {
                    if fat.weapon_ammo == u32::MAX {
                        w.ammo = None;
                    } else {
                        w.ammo = Some(fat.weapon_ammo);
                    }
                    if fat.weapon_clip_size > 0 {
                        w.clip_size = fat.weapon_clip_size;
                    }
                }
            }
            if !skip_ai {
                obj.attack_substate =
                    crate::game_logic::AttackSubState::from_ordinal(fat.attack_substate_ordinal);
                obj.ai_state = crate::gameworld_shadow::GameWorldShadow::ai_state_from_ordinal(
                    fat.ai_state_ordinal,
                );
            }
            if !skip_contain {
                obj.contained_by = if fat.contained_by_host == 0 {
                    None
                } else {
                    Some(ObjectId(fat.contained_by_host))
                };
                if !fat.garrisoned_host_ids.is_empty() || !obj.occupants.is_empty() {
                    obj.occupants = fat
                        .garrisoned_host_ids
                        .iter()
                        .copied()
                        .map(ObjectId)
                        .collect();
                }
            }
            if !skip_move {
                obj.movement.target_position =
                    fat.move_target.map(|p| glam::Vec3::new(p[0], p[1], p[2]));
                if !fat.path_waypoints.is_empty() {
                    obj.movement.path = fat
                        .path_waypoints
                        .iter()
                        .map(|p| glam::Vec3::new(p[0], p[1], p[2]))
                        .collect();
                    obj.movement.current_path_index = fat.path_index as usize;
                } else {
                    obj.movement.path.clear();
                    obj.movement.current_path_index = 0;
                }
            }
        }
    }

    /// Refresh every mapped object from GameWorld so the HashMap is a view.
    pub fn sync_authoritative_view_from_gameworld(&mut self) {
        if !crate::gameworld_shadow::gameworld_shadow_enabled() {
            return;
        }
        if crate::gameworld_shadow::with_active_shadow(|_| ()).is_none() {
            return;
        }
        let ids: Vec<ObjectId> = self.objects.keys().copied().collect();
        for id in ids {
            self.overlay_object_from_gameworld(id);
        }
    }

    /// Drain the dirty set. Coupled shadow: no-op — phases 0–6 last-write via
    /// logs/mutations; the HashMap is a read-view. Uncoupled: push residual.
    pub fn commit_dirty_host_objects_to_gameworld(&mut self) {
        let dirty = std::mem::take(&mut self.host_view_dirty);
        if crate::gameworld_shadow::shadow_coupled_tick_active() {
            return;
        }
        for id in dirty {
            self.push_coupled_host_object_mutations(id);
        }
    }

    /// Push HP/pose/target from the host ID-map view into GameWorld (coupled only).
    fn push_coupled_host_object_mutations(&self, id: ObjectId) {
        use crate::gameworld_shadow::{push_coupled_world_mutation, with_active_shadow};
        use gamelogic::world::WorldMutation;
        use gamelogic::world::entities::EntityProductionItem;
        let Some(obj) = self.host_objects().get(&id) else {
            return;
        };
        let Some(eid) = with_active_shadow(|s| s.entity_for_host(id)).flatten() else {
            return;
        };
        let pos = obj.get_position();
        // Mid-frame damage logs own HP until writeback; do not stomp GameWorld.
        if !crate::game_logic::host_damage_log::has_pending(id) {
            let _ = push_coupled_world_mutation(WorldMutation::SetHealth {
                target: eid,
                health: obj.health.current,
            });
        }
        let _ = push_coupled_world_mutation(WorldMutation::SetTransform {
            target: eid,
            position: [pos.x, pos.y, pos.z],
            orientation: obj.get_orientation(),
        });
        let gw_target = obj
            .target
            .and_then(|tid| with_active_shadow(|s| s.entity_for_host(tid)).flatten());
        let _ = push_coupled_world_mutation(WorldMutation::SetAttackTarget {
            attacker: eid,
            target: gw_target,
        });
        if !crate::game_logic::host_movement_log::has_pending(id) {
            let _ = push_coupled_world_mutation(WorldMutation::SetMoveTarget {
                unit: eid,
                destination: obj
                    .movement
                    .target_position
                    .map(|dest| [dest.x, dest.y, dest.z]),
            });
            let path_waypoints: Vec<[f32; 3]> =
                obj.movement.path.iter().map(|p| [p.x, p.y, p.z]).collect();
            let _ = push_coupled_world_mutation(WorldMutation::SetMovement {
                target: eid,
                velocity: [
                    obj.movement.velocity.x,
                    obj.movement.velocity.y,
                    obj.movement.velocity.z,
                ],
                max_speed: obj.movement.max_speed,
                path_index: obj.movement.current_path_index.min(u16::MAX as usize) as u16,
                path_len: obj.movement.path.len().min(u16::MAX as usize) as u16,
                path_waypoints,
                waiting_for_path: obj.waiting_for_path,
                locomotor_surfaces: obj.locomotor_surfaces,
                is_attack_path: obj.is_attack_path,
                is_blocked_and_stuck: obj.is_blocked_and_stuck,
                is_braking: obj.is_braking,
                is_safe_path: obj.is_safe_path,
                queue_for_path_frames: obj.queue_for_path_frames,
                path_timestamp: obj.path_timestamp,
                cur_max_blocked_speed: obj.cur_max_blocked_speed,
                num_frames_blocked: obj.num_frames_blocked,
                is_blocked: obj.is_blocked,
                move_away_from_id: obj.move_away_from.map(|i| i.0),
                requested_victim_id: obj.requested_victim_id.map(|i| i.0),
            });
        }
        if !crate::game_logic::host_weapon_stats_log::has_pending(id) {
            if let Some(w) = obj.weapon.as_ref() {
                let sec = obj.secondary_weapon.as_ref();
                let _ = push_coupled_world_mutation(WorldMutation::SetWeaponStats {
                    target: eid,
                    has_weapon: true,
                    weapon_damage: w.damage,
                    weapon_range: w.range,
                    weapon_min_range: w.min_range,
                    weapon_reload_time: w.reload_time,
                    weapon_last_fire_time: w.last_fire_time,
                    weapon_clip_size: w.clip_size,
                    weapon_clip_reload_time: w.clip_reload_time,
                    weapon_ammo: w.ammo.unwrap_or(u32::MAX),
                    weapon_can_target_air: w.can_target_air,
                    weapon_can_target_ground: w.can_target_ground,
                    weapon_projectile_speed: w.projectile_speed,
                    has_secondary_weapon: sec.is_some(),
                    secondary_weapon_damage: sec.map(|s| s.damage).unwrap_or(0.0),
                    secondary_weapon_range: sec.map(|s| s.range).unwrap_or(0.0),
                    leech_range_active_primary: obj.leech_range_active_primary,
                    leech_range_active_secondary: obj.leech_range_active_secondary,
                });
            }
            use gamelogic::world::{
                WEAPON_SLOT_MINE_CLEAR, WEAPON_SLOT_PRIMARY, WEAPON_SLOT_SECONDARY,
                WEAPON_SLOT_TERTIARY, WeaponSlotFacts,
            };
            let slot_facts = |slot: u8, w: Option<&crate::game_logic::Weapon>| {
                w.map(|w| WeaponSlotFacts {
                    present: true,
                    clip_size: w.clip_size,
                    ammo: w.ammo.unwrap_or(u32::MAX),
                    reload_time: w.reload_time,
                    last_fire_time: w.last_fire_time,
                    barrel_cursor: obj
                        .weapon_barrel_states
                        .get(slot as usize)
                        .map(|b| b.current_barrel)
                        .unwrap_or(0),
                    barrel_count: obj
                        .weapon_barrel_states
                        .get(slot as usize)
                        .map(|b| b.barrel_count)
                        .unwrap_or(0),
                    lock_type: if obj.weapon_lock_slot == slot {
                        obj.weapon_lock_type as u8
                    } else {
                        0
                    },
                })
            };
            for (slot, w) in [
                (WEAPON_SLOT_PRIMARY, obj.weapon.as_ref()),
                (WEAPON_SLOT_SECONDARY, obj.secondary_weapon.as_ref()),
                (WEAPON_SLOT_TERTIARY, obj.tertiary_weapon.as_ref()),
                (
                    WEAPON_SLOT_MINE_CLEAR,
                    obj.mine_clearing_primary_weapon.as_ref(),
                ),
            ] {
                if let Some(facts) = slot_facts(slot, w) {
                    let _ = push_coupled_world_mutation(WorldMutation::SetWeaponSlot {
                        target: eid,
                        slot,
                        facts,
                    });
                }
            }
        }
        if !crate::game_logic::host_combat_attack_log::has_pending(id)
            && !crate::game_logic::host_ai_state_log::has_pending(id)
        {
            let _ = push_coupled_world_mutation(WorldMutation::SetAiState {
                target: eid,
                ordinal: crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(
                    &obj.ai_state,
                ),
            });
            let _ = push_coupled_world_mutation(WorldMutation::SetCombatAttack {
                target: eid,
                pre_attack_target_host: obj.pre_attack_target.map(|t| t.0).unwrap_or(0),
                pre_attack_ready_at: obj.pre_attack_ready_at,
                consecutive_shots_at_target: obj.consecutive_shots_at_target,
                max_shots_to_fire: obj.max_shots_to_fire,
                attack_substate_ordinal: obj.attack_substate.to_ordinal(),
                approach_timestamp: obj.approach_timestamp,
                continuous_fire_victim: obj.continuous_fire_victim,
                maintain_pos_valid: obj.maintain_pos_valid,
                maintain_pos: obj.maintain_pos.map(|p| [p.x, p.y, p.z]),
                temporary_move_frames: obj.temporary_move_frames,
                group_speed_factor: obj.group_speed_factor,
            });
        }
        if !crate::game_logic::host_contain_log::has_pending(id) {
            let _ = push_coupled_world_mutation(WorldMutation::SetContain {
                target: eid,
                contained_by_host: obj.contained_by.map(|c| c.0).unwrap_or(0),
                garrison_count: Some(obj.occupants.len().min(u16::MAX as usize) as u16),
                garrisoned_host_ids: Some(obj.occupants.iter().map(|o| o.0).collect()),
            });
        }
        if crate::gameworld_shadow::gameworld_production_authority_enabled()
            && !crate::gameworld_shadow::gameworld_production_sole_tick_enabled()
            && !crate::game_logic::host_production_log::has_pending(id)
        {
            if let Some(bd) = obj.building_data.as_ref() {
                let items: Vec<EntityProductionItem> = bd
                    .production_queue
                    .iter()
                    .take(16)
                    .map(|it| EntityProductionItem {
                        template_name: it.template_name.clone(),
                        progress: it.progress,
                        total_time: it.total_time,
                        construction_frames: it.construction_frames,
                        cost_supplies: it.cost.supplies,
                        is_upgrade: it.is_upgrade(),
                        quantity_total: it.quantity_total.max(1),
                        quantity_produced: it.quantity_produced,
                    })
                    .collect();
                let _ = push_coupled_world_mutation(WorldMutation::SetProductionQueue {
                    target: eid,
                    items,
                });
                if bd.queue_exit_state_initialized {
                    let _ = push_coupled_world_mutation(WorldMutation::SetProductionExitRuntime {
                        target: eid,
                        exit_delay_remaining_frames: bd.exit_delay_remaining_frames,
                        exit_burst_remaining: bd.exit_burst_remaining,
                        queue_exit_state_initialized: true,
                    });
                }
            }
            let _ = push_coupled_world_mutation(WorldMutation::SetProductionDoor {
                target: eid,
                production_door_phase: obj.production_door_phase,
                production_door_phase_end_frame: obj.production_door_phase_end_frame,
                production_door_hold_open: obj.production_door_hold_open,
            });
        }
        if crate::gameworld_shadow::gameworld_construction_authority_enabled()
            && !crate::gameworld_shadow::gameworld_construction_sole_tick_enabled()
            && !crate::game_logic::host_construction_log::has_pending(id)
        {
            let _ = push_coupled_world_mutation(WorldMutation::SetConstruction {
                target: eid,
                percent: obj.construction_percent.clamp(-1.0, 1.0),
                under_construction: obj.status.under_construction,
            });
        }
    }

    /// Authoritative weapon clip ammo (GameWorld when coupled).
    pub fn host_authoritative_weapon_ammo(&self, id: ObjectId) -> Option<u32> {
        if let Some(a) = crate::gameworld_shadow::coupled_entity_weapon_ammo(id) {
            if a != u32::MAX {
                return Some(a);
            }
        }
        self.host_object(id)
            .and_then(|o| o.weapon.as_ref())
            .and_then(|w| w.ammo)
    }

    /// Freeze the C++ `Drawable::updateDrawableClipStatus` arguments for all
    /// concrete host WeaponSet slots.
    ///
    /// A coupled GameWorld currently owns one primary weapon clip record.  It
    /// is authoritative while present, so Main exposes only that exact pair
    /// and deliberately leaves secondary/tertiary unset rather than mixing a
    /// stale host mirror into a live GameWorld presentation frame.  The
    /// uncoupled host retains all three slots and reports each independently,
    /// matching Object::adjustModelConditionForWeaponStatus iteration order.
    pub fn host_authoritative_projectile_clip_statuses(
        &self,
        id: ObjectId,
    ) -> [Option<(u32, u32)>; 3] {
        let valid_clip = |shots_remaining: u32, max_shots: u32| {
            (max_shots > 0 && shots_remaining <= max_shots).then_some((shots_remaining, max_shots))
        };

        if let Some(fat) = crate::gameworld_shadow::coupled_entity_fat_view(id) {
            // C++ Weapon::getRemainingAmmo reports zero while a reloaded
            // clip's backing counter has already been reset to full.
            let shots_remaining = if fat.active_weapon_slot == 0
                && fat.weapon_fire_status
                    == crate::game_logic::object::WeaponFireStatus::ReloadingClip as u8
            {
                0
            } else {
                fat.weapon_ammo
            };
            return [
                (fat.weapon_ammo != u32::MAX)
                    .then(|| valid_clip(shots_remaining, fat.weapon_clip_size))
                    .flatten(),
                None,
                None,
            ];
        }

        let Some(object) = self.host_object(id) else {
            return [None; 3];
        };
        std::array::from_fn(|index| {
            let slot = u8::try_from(index).ok()?;
            let weapon = object.weapon_slot(slot)?;
            let shots_remaining = if weapon.reloading_clip
                || (slot == object.active_weapon_slot
                    && object.weapon_fire_status
                        == crate::game_logic::object::WeaponFireStatus::ReloadingClip)
            {
                0
            } else {
                weapon.ammo?
            };
            valid_clip(shots_remaining, weapon.clip_size)
        })
    }

    /// Mutate attack substate via HashMap field borrow so `self.frame` stays readable.
    pub fn host_stamp_attack_substate_at_frame(
        &mut self,
        id: ObjectId,
        sub: crate::game_logic::AttackSubState,
    ) -> Option<u32> {
        let frame = self.frame;
        let obj = self.objects.get_mut(&id)?;
        obj.attack_substate = sub;
        obj.approach_timestamp = frame;
        if !crate::gameworld_shadow::shadow_coupled_tick_active() {
            self.host_view_dirty.insert(id);
        }
        Some(frame)
    }

    /// Authoritative AttackStateMachine substate ordinal (GameWorld when coupled).
    pub fn host_authoritative_attack_substate(&self, id: ObjectId) -> Option<u8> {
        if let Some(s) = crate::gameworld_shadow::coupled_entity_attack_substate(id) {
            return Some(s);
        }
        self.host_object(id).map(|o| o.attack_substate.to_ordinal())
    }

    /// Authoritative occupant/garrison count (GameWorld when coupled).
    /// TunnelContain redirects to the controlling player's TunnelTracker.
    pub fn host_authoritative_occupant_count(&self, id: ObjectId) -> Option<u16> {
        if let Some(n) = crate::gameworld_shadow::coupled_entity_occupant_count(id) {
            return Some(n);
        }
        let o = self.host_object(id)?;
        if o.is_tunnel_network_style_container()
            || crate::game_logic::host_tunnel_network::is_tunnel_network_template(&o.template_name)
        {
            return Some(
                self.tunnel_network
                    .contain_count(o.tunnel_system_key())
                    .min(u16::MAX as usize) as u16,
            );
        }
        Some(o.occupants.len().min(u16::MAX as usize) as u16)
    }

    /// Authoritative contained-unit list. Tunnels export the shared player pool
    /// so every entrance shows the same ControlBar inventory.
    pub fn host_authoritative_contained_units(&self, id: ObjectId) -> Vec<ObjectId> {
        let Some(o) = self.host_object(id) else {
            return Vec::new();
        };
        if o.is_tunnel_network_style_container()
            || crate::game_logic::host_tunnel_network::is_tunnel_network_template(&o.template_name)
        {
            return self
                .tunnel_network
                .contained_for_player(o.tunnel_system_key());
        }
        o.contained_units()
    }

    /// Authoritative move destination (GameWorld when coupled).
    ///
    /// If a fat view is mapped, `None` dest is authoritative (stopped unit).
    /// Do not fall back to a disagreeing HashMap dest.
    pub fn host_authoritative_move_dest(&self, id: ObjectId) -> Option<[f32; 3]> {
        if let Some(fat) = crate::gameworld_shadow::coupled_entity_fat_view(id) {
            return fat.move_target;
        }
        self.host_object(id)
            .and_then(|o| o.movement.target_position)
            .map(|p| [p.x, p.y, p.z])
    }

    /// Authoritative HP: GameWorld when the coupled session is live, else host field.
    pub fn host_authoritative_health(&self, id: ObjectId) -> Option<f32> {
        if let Some(h) = crate::gameworld_shadow::coupled_entity_health(id) {
            return Some(h);
        }
        self.host_object(id).map(|o| o.health.current)
    }

    /// Authoritative construction: `(percent, under_construction)`.
    /// GameWorld when mapped, else host HashMap fields.
    /// Fraction is host/GW 0–1; use `host_authoritative_construction_cpp` for 0–100.
    pub fn host_authoritative_construction(&self, id: ObjectId) -> Option<(f32, bool)> {
        if let Some(c) = crate::gameworld_shadow::coupled_entity_construction(id) {
            return Some(c);
        }
        self.host_object(id)
            .map(|o| (o.construction_percent, o.status.under_construction))
    }

    /// C++ construction percent (0–100, −1 complete, −50 sell). Converts at the GW 0–1 boundary.
    pub fn host_authoritative_construction_cpp(&self, id: ObjectId) -> Option<(i32, bool)> {
        let selling = self.host_object(id).map(|o| o.status.sold).unwrap_or(false);
        self.host_authoritative_construction(id).map(|(frac, uc)| {
            (
                crate::game_logic::host_production_buildable_command_residual::host_fraction_to_cpp_construction_percent(
                    frac, uc, selling,
                ),
                uc,
            )
        })
    }

    /// Authoritative pose: GameWorld when the coupled session is live, else host field.
    pub fn host_authoritative_pose(&self, id: ObjectId) -> Option<[f32; 3]> {
        if let Some(p) = crate::gameworld_shadow::coupled_entity_pose(id) {
            return Some(p);
        }
        self.host_object(id).map(|o| {
            let p = o.get_position();
            [p.x, p.y, p.z]
        })
    }

    /// Authoritative cash: GameWorld when the coupled session is live, else host field.
    pub fn host_authoritative_cash(&self, player_id: u32) -> Option<u32> {
        if let Some(c) = crate::gameworld_shadow::coupled_player_cash(player_id) {
            return Some(c);
        }
        self.get_player(player_id).map(|p| p.resources.supplies)
    }

    /// Authoritative attack target: GameWorld when coupled, else host field.
    ///
    /// If a fat view is mapped, `None` target is authoritative (no target).
    /// Do not fall back to a disagreeing HashMap target.
    pub fn host_authoritative_target(&self, id: ObjectId) -> Option<ObjectId> {
        if crate::gameworld_shadow::coupled_entity_fat_view(id).is_some() {
            return crate::gameworld_shadow::coupled_entity_target_host(id);
        }
        self.host_object(id).and_then(|o| o.target)
    }

    /// Wave 955/958: host-authority object borrow (preferred over get_object dual-read).
    /// When shadow is coupled this HashMap is an ID map / read-through view of
    /// GameWorld; use `host_authoritative_*` for HP/pose/cash/target.
    #[inline]
    pub fn host_object(&self, id: ObjectId) -> Option<&crate::game_logic::object::Object> {
        self.objects.get(&id)
    }

    /// Wave 955/958: host-authority object map borrow (command apply / AI / shadow).
    /// Presentation dual-read paths must use `PresentationFrame`, not this.
    #[inline]
    pub fn host_objects(
        &self,
    ) -> &std::collections::HashMap<ObjectId, crate::game_logic::object::Object> {
        self.objects.map()
    }

    /// Wave 955/958: host-authority mutable object map borrow.
    #[inline]
    pub fn host_objects_mut(
        &mut self,
    ) -> &mut std::collections::HashMap<ObjectId, crate::game_logic::object::Object> {
        self.objects.map_mut()
    }

    /// Wave 950/958: host-authority mutable object borrow.
    #[inline]
    pub fn host_object_mut(
        &mut self,
        id: ObjectId,
    ) -> Option<&mut crate::game_logic::object::Object> {
        self.overlay_object_from_gameworld(id);
        if !crate::gameworld_shadow::shadow_coupled_tick_active() && self.objects.contains_key(&id)
        {
            self.host_view_dirty.insert(id);
        }
        self.objects.get_mut(&id)
    }

    /// Issue attack command to selected objects
    pub fn command_attack(&mut self, player_id: u32, target_id: ObjectId) {
        if let Some(player) = self.players.get(&player_id) {
            if self.objects.get(&target_id).is_none() {
                return;
            }

            let selected: Vec<ObjectId> = player
                .selected_objects
                .iter()
                .copied()
                .filter(|object_id| {
                    self.objects
                        .get(object_id)
                        .is_some_and(|obj| obj.owner_player_id == Some(player_id))
                })
                .collect();
            let mut accepted_attackers = Vec::new();
            for &object_id in &selected {
                if self.flight_deck_ai_do_command(
                    object_id,
                    crate::game_logic::host_flight_deck::HostFlightDeckCommand::AttackObject,
                    Some(target_id),
                    None,
                ) {
                    accepted_attackers.push(object_id);
                    continue;
                }

                // This is the default host-authority route used by actual
                // WND/right-click orders.  It must not bypass the same C++
                // WeaponSet legality used by the typed command executor:
                // MASKED/UNATTACKABLE, stealth, relationship, concrete
                // Weapon.ini Anti* mask, and range all apply before we stamp
                // a target or issue a movement path.
                if !matches!(
                    self.get_able_to_attack_specific_object(
                        object_id,
                        target_id,
                        AbleToAttackType::NewTarget,
                        true,
                    ),
                    CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
                ) {
                    continue;
                }
                accepted_attackers.push(object_id);

                // Host attack channel (default production path — host ObjectIds only).
                if let Some(obj_mut) = self.objects.get_mut(&object_id) {
                    obj_mut.set_force_attack(false);
                    obj_mut.attack_target(target_id);
                }
                // Host residual: path toward target, then ensure the unit is inside
                // weapon range this command so combat can apply real HP damage on
                // large maps (path marches otherwise take longer than smoke waits).
                if let Some(tpos) = self.objects.get(&target_id).map(|t| t.get_position()) {
                    let _ = self.assign_unit_attack_path(object_id, Some(target_id), tpos);
                    if let Some(attacker) = self.objects.get(&object_id) {
                        if !attacker.can_attack() {
                            // leave unarmed units alone
                        } else {
                            let range = attacker
                                .weapon
                                .as_ref()
                                .map(|w| w.range)
                                .or_else(|| attacker.secondary_weapon.as_ref().map(|w| w.range))
                                .unwrap_or(50.0)
                                .max(15.0);
                            let from = attacker.get_position();
                            let mut dir = tpos - from;
                            dir.y = 0.0;
                            let dist = dir.length();
                            if dist > range * 0.8 {
                                // Movement authority: no range-snap teleport. Path was
                                // already issued via assign_unit_attack_path; GameWorld
                                // integrates the march. Host-only residual may still snap
                                // for short smoke waits when authority is off.
                                if !crate::gameworld_shadow::gameworld_movement_authority_live() {
                                    let dir = if dist > 1.0 {
                                        dir / dist
                                    } else {
                                        glam::Vec3::new(1.0, 0.0, 0.0)
                                    };
                                    let stand = tpos - dir * (range * 0.55);
                                    let stand = glam::Vec3::new(stand.x, from.y, stand.z);
                                    if let Some(a) = self.objects.get_mut(&object_id) {
                                        a.set_position(stand);
                                        a.attack_target(target_id);
                                        // Host-immediate engagement residual (host-only
                                        // path when movement auth is off).
                                        a.set_ai_state(AIState::Attacking);
                                        if crate::gameworld_shadow::gameworld_ai_decision_authority_live()
                                        {
                                            crate::game_logic::host_ai_decision_log::record_set_state(
                                                object_id, 2,
                                            );
                                        }
                                        a.set_status_attacking(true);
                                        a.set_status_moving(false);
                                        a.movement.velocity = glam::Vec3::ZERO;
                                        a.record_host_movement();
                                        a.movement.target_position = None;
                                        a.movement.path.clear();
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // C++ VoiceAttack / VoiceAttackAir from ThingTemplate INI (not UnitVoiceAttack).
            let local = self
                .players
                .get(&player_id)
                .map(|p| p.is_local)
                .unwrap_or(false);
            if local {
                self.queue_attack_voice(&accepted_attackers, Some(target_id), false, false, None);
            }
            log::trace!(
                "{} commanded {} units to attack object {}",
                player_id,
                accepted_attackers.len(),
                target_id
            );
        }
    }

    /// C++ pickAndPlay attack branch: VoiceAttack/Air, then specialty weapon upgrade.
    pub fn queue_attack_voice(
        &mut self,
        unit_ids: &[ObjectId],
        target_id: Option<ObjectId>,
        specialty_weapon: bool,
        at_location: bool,
        forced_slot: Option<u8>,
    ) {
        use crate::game_logic::audio_dispatch_impl::{
            AttackVoiceWeapon, UnitVoiceSlot, pick_specialty_attack_voice,
        };
        let air = target_id.is_some_and(|tid| {
            self.objects
                .get(&tid)
                .is_some_and(|t| t.is_kind_of(KindOf::Aircraft) || t.status.airborne_target)
        });
        let structure = target_id.is_some_and(|tid| {
            self.objects
                .get(&tid)
                .is_some_and(|t| t.is_kind_of(KindOf::Structure))
        });
        let default = if air {
            UnitVoiceSlot::AttackAir
        } else {
            UnitVoiceSlot::Attack
        };
        let weapons: Vec<AttackVoiceWeapon> = unit_ids
            .iter()
            .filter_map(|&id| {
                let obj = self.objects.get(&id)?;
                if obj.is_kind_of(KindOf::IgnoredInGui) {
                    return None;
                }
                let slot = forced_slot
                    .or_else(|| obj.selected_weapon_slot())
                    .unwrap_or(obj.active_weapon_slot);
                let name = obj.weapon_name_for_slot(slot)?.to_string();
                Some(AttackVoiceWeapon { name, slot })
            })
            .collect();
        let slot =
            pick_specialty_attack_voice(default, weapons, structure, specialty_weapon, at_location);
        self.queue_picked_unit_voice(unit_ids, slot);
    }

    /// C++ MSG_DO_MOVETO / ATTACKMOVETO / GET_REPAIRED / GET_HEALED (`CommandXlat.cpp:384-443`).
    /// Worker Shoes complete + infantry/dozer/harvester → PerUnitSound VoiceMoveUpgraded (skip).
    pub fn queue_picked_move_voice(&mut self, unit_ids: &[ObjectId]) {
        if self.try_queue_picked_voice_move_upgraded(unit_ids) {
            return;
        }
        self.queue_picked_unit_voice(
            unit_ids,
            crate::game_logic::audio_dispatch_impl::UnitVoiceSlot::Move,
        );
    }

    /// C++ `CommandXlat.cpp:433-443` — first matching worker wins (`skip=true`).
    /// Always plays the PerUnitSound key; C++ does not `isValidAudioEvent` this line.
    pub fn try_queue_picked_voice_move_upgraded(&mut self, unit_ids: &[ObjectId]) -> bool {
        use crate::game_logic::audio_dispatch_impl::resolve_per_unit_sound;
        use crate::game_logic::host_gla_worker::{
            UPGRADE_GLA_WORKER_SHOES, WORKER_VOICE_MOVE_UPGRADED,
            is_worker_for_voice_move_upgraded, worker_shoes_voice_upgrade_complete,
        };
        for &id in unit_ids {
            let Some(obj) = self.objects.get(&id) else {
                continue;
            };
            if obj.is_kind_of(KindOf::IgnoredInGui) {
                continue;
            }
            if !is_worker_for_voice_move_upgraded(
                obj.is_kind_of(KindOf::Infantry),
                obj.is_kind_of(KindOf::Dozer),
                obj.is_kind_of(KindOf::Harvester),
                &obj.template_name,
            ) {
                continue;
            }
            let object_has_shoes = obj.has_upgrade_tag(UPGRADE_GLA_WORKER_SHOES);
            let player_names = |p: &crate::game_logic::Player| {
                p.has_unlocked_upgrade(UPGRADE_GLA_WORKER_SHOES)
                    || worker_shoes_voice_upgrade_complete(
                        false,
                        p.unlocked_sciences
                            .iter()
                            .chain(p.completed_upgrades.iter()),
                    )
            };
            let player_has_shoes = obj
                .owner_player_id
                .and_then(|pid| self.players.get(&pid))
                .map(player_names)
                .unwrap_or_else(|| {
                    self.players
                        .values()
                        .any(|p| p.team == obj.team && player_names(p))
                });
            if !object_has_shoes && !player_has_shoes {
                continue;
            }
            let event = resolve_per_unit_sound(&obj.template_name, WORKER_VOICE_MOVE_UPGRADED)
                .unwrap_or_else(|| WORKER_VOICE_MOVE_UPGRADED.to_string());
            let pos = obj.get_position();
            self.queue_audio_event(
                AudioEventRequest::new(&event)
                    .with_object(id)
                    .with_position(pos)
                    .with_priority(100),
            );
            return true;
        }
        false
    }

    /// C++ `pickAndPlayUnitVoiceResponse` — authored Voice.ini plus carbomb extra.
    pub fn queue_picked_unit_voice(
        &mut self,
        unit_ids: &[ObjectId],
        slot: crate::game_logic::audio_dispatch_impl::UnitVoiceSlot,
    ) {
        use crate::game_logic::audio_dispatch_impl::{
            resolve_terrorist_in_car_voice, resolve_unit_voice_event,
        };
        let mut chosen: Option<(String, ObjectId, glam::Vec3, bool)> = None;
        for &id in unit_ids {
            let Some(obj) = self.objects.get(&id) else {
                continue;
            };
            if obj.is_kind_of(KindOf::IgnoredInGui) {
                continue;
            }
            let Some(event) = resolve_unit_voice_event(&obj.template_name, slot) else {
                continue;
            };
            chosen = Some((event, id, obj.get_position(), obj.status.is_carbomb));
            if slot.first_unit_wins() {
                break;
            }
        }
        if let Some((event, id, pos, is_carbomb)) = chosen {
            self.queue_audio_event(
                AudioEventRequest::new(&event)
                    .with_object(id)
                    .with_position(pos)
                    .with_priority(100),
            );
            // C++ CommandXlat.cpp:690-728 — IS_CARBOMB layers MiscAudio TerroristInCar*.
            if is_carbomb {
                if let Some(extra) = resolve_terrorist_in_car_voice(slot) {
                    self.queue_audio_event(
                        AudioEventRequest::new(&extra)
                            .with_object(id)
                            .with_position(pos)
                            .with_priority(100),
                    );
                }
            }
        }
    }
}

/// C++ `Object::isMassSelectable` (`Object.cpp:3024-3026`):
/// `isSelectable() && !isKindOf(KINDOF_STRUCTURE)`.
fn host_object_is_mass_selectable(obj: &Object) -> bool {
    obj.is_selectable() && !obj.is_kind_of(KindOf::Structure)
}

#[cfg(test)]
mod select_object_cpp_parity_tests {
    use super::*;
    use crate::game_logic::audio_dispatch_impl::{
        UnitVoiceSlot, clear_test_template_voices, set_test_template_voice,
    };
    use crate::game_logic::{KindOf, ObjectId, Player, Team, ThingTemplate};
    use glam::Vec3;

    const TEST_VOICE_SELECT: &str = "AmericaRangerVoiceSelect";

    fn bind_select_voice() {
        set_test_template_voice("SelectParityUnit", UnitVoiceSlot::Select, TEST_VOICE_SELECT);
    }

    fn queued_select_voices(logic: &GameLogic) -> Vec<&str> {
        logic
            .queued_audio_events
            .iter()
            .filter(|e| e.event_type == TEST_VOICE_SELECT)
            .map(|e| e.event_type.as_str())
            .collect()
    }

    fn ensure_tpl(logic: &mut GameLogic, name: &str, kinds: &[KindOf]) {
        if logic.templates.contains_key(name) {
            return;
        }
        let mut t = ThingTemplate::new(name);
        t.set_health(100.0);
        for kind in kinds {
            t.add_kind_of(*kind);
        }
        logic.templates.insert(name.to_string(), t);
    }

    fn two_player_logic() -> GameLogic {
        let mut logic = GameLogic::new();
        logic.clear_all_players();
        logic.add_player(Player::new(0, Team::USA, "local", true));
        logic.add_player(Player::new(1, Team::China, "other", false));
        ensure_tpl(
            &mut logic,
            "SelectParityUnit",
            &[KindOf::Infantry, KindOf::Selectable],
        );
        ensure_tpl(
            &mut logic,
            "SelectParityBuilding",
            &[KindOf::Structure, KindOf::Selectable],
        );
        logic
    }

    fn spawn(logic: &mut GameLogic, name: &str, owner: u32, x: f32) -> ObjectId {
        logic
            .create_object_for_player(name, owner, Vec3::new(x, 0.0, 0.0))
            .expect("spawn")
    }

    #[test]
    fn select_objects_allows_non_owner_on_create_new() {
        // C++ GameLogic::selectObject (GameLogic.cpp:2595-2641) has no owner
        // test — playerMask is the only player predicate.
        let mut logic = two_player_logic();
        let mine = spawn(&mut logic, "SelectParityUnit", 0, 0.0);
        let theirs = spawn(&mut logic, "SelectParityUnit", 1, 20.0);

        logic.select_objects(0, vec![mine, theirs]);
        assert_eq!(
            logic.get_player(0).expect("p0").selected_objects,
            vec![mine, theirs],
            "createNewSelection must keep enemy/neutral objects in the mask player's list"
        );
        assert!(logic.host_object(theirs).expect("theirs").selected);
    }

    #[test]
    fn hidden_drawable_deselects_like_cpp_update_hidden_status() {
        let mut logic = two_player_logic();
        let unit = spawn(&mut logic, "SelectParityUnit", 0, 0.0);
        logic.select_objects(0, vec![unit]);
        assert!(logic.host_object(unit).expect("u").selected);
        if let Some(obj) = logic.host_object_mut(unit) {
            obj.set_drawable_hidden(true);
        }
        logic.drain_hidden_drawable_selection();
        assert!(
            !logic.host_object(unit).expect("u").selected,
            "C++ updateHiddenStatus deselects a hidden drawable"
        );
        assert!(
            logic.get_player(0).expect("p0").selected_objects.is_empty(),
            "TheInGameUI->deselectDrawable must drop the player list"
        );

        if let Some(obj) = logic.host_object_mut(unit) {
            obj.set_drawable_hidden(false);
            obj.select();
            obj.camo_stealth_look = 5;
            obj.update_drawable_hidden_status();
        }
        logic.drain_hidden_drawable_selection();
        assert!(
            !logic.host_object(unit).expect("u").selected,
            "STEALTHLOOK_INVISIBLE must leftover-deselect"
        );
    }

    #[test]
    fn mask_object_deselects_like_cpp_mask_object() {
        let mut logic = two_player_logic();
        let unit = spawn(&mut logic, "SelectParityUnit", 0, 0.0);
        logic.select_objects(0, vec![unit]);
        assert!(logic.host_object(unit).expect("u").selected);
        if let Some(obj) = logic.host_object_mut(unit) {
            obj.set_status_masked(true);
        }
        logic.drain_masked_object_selection();
        assert!(
            logic.host_object(unit).expect("u").status.masked,
            "maskObject sets OBJECT_STATUS_MASKED"
        );
        assert!(
            !logic.host_object(unit).expect("u").selected,
            "C++ maskObject deselects when masking"
        );
        assert!(
            logic.get_player(0).expect("p0").selected_objects.is_empty(),
            "deselectObject must drop the player list"
        );
    }

    #[test]
    fn is_hero_includes_contained_kindof_hero() {
        let mut logic = two_player_logic();
        ensure_tpl(
            &mut logic,
            "SelectParityHero",
            &[KindOf::Infantry, KindOf::Selectable, KindOf::Hero],
        );
        let humvee = spawn(&mut logic, "SelectParityUnit", 0, 0.0);
        let burton = spawn(&mut logic, "SelectParityHero", 0, 10.0);
        assert!(logic.unit_is_hero(burton));
        assert!(!logic.unit_is_hero(humvee));
        if let Some(obj) = logic.host_object_mut(humvee) {
            assert!(obj.add_occupant(burton));
        }
        assert!(
            logic.unit_is_hero(humvee),
            "C++ isHero is true when a contained occupant is KINDOF_HERO"
        );
    }

    #[test]
    fn select_object_list_loops_every_player_in_mask() {
        // C++ GameLogic.cpp:2608-2629 — getEachPlayerFromMask then
        // setCurrentlySelectedAIGroup / addAIGroupToCurrentSelection.
        let mut logic = two_player_logic();
        let unit = spawn(&mut logic, "SelectParityUnit", 0, 0.0);

        logic.select_object_list(0b11, vec![unit], true);
        assert_eq!(
            logic.get_player(0).expect("p0").selected_objects,
            vec![unit]
        );
        assert_eq!(
            logic.get_player(1).expect("p1").selected_objects,
            vec![unit]
        );
    }

    #[test]
    fn add_to_selection_rejects_non_mass_selectable_structure() {
        // C++ GameLogic.cpp:2602-2606 — !isMassSelectable && !createNewSelection
        // returns without adding. Object.cpp:3024: structures are not mass-selectable.
        let mut logic = two_player_logic();
        let unit = spawn(&mut logic, "SelectParityUnit", 0, 0.0);
        let building = spawn(&mut logic, "SelectParityBuilding", 0, 30.0);

        logic.select_objects(0, vec![unit]);
        logic.select_object_list(1, vec![building], false);
        assert_eq!(
            logic.get_player(0).expect("p0").selected_objects,
            vec![unit],
            "structure must not join an existing selection"
        );
        assert!(
            !logic.host_object(building).expect("building").selected,
            "rejected add must not mark the structure selected"
        );
    }

    #[test]
    fn create_new_selection_allows_non_mass_selectable_structure() {
        // C++ GameLogic.cpp:2602 — the mass-selectable gate is skipped when
        // createNewSelection is true (clicking a building replaces selection).
        let mut logic = two_player_logic();
        let unit = spawn(&mut logic, "SelectParityUnit", 0, 0.0);
        let building = spawn(&mut logic, "SelectParityBuilding", 0, 30.0);

        logic.select_objects(0, vec![unit]);
        logic.select_object_list(1, vec![building], true);
        assert_eq!(
            logic.get_player(0).expect("p0").selected_objects,
            vec![building]
        );
        assert!(logic.host_object(building).expect("building").selected);
        assert!(!logic.host_object(unit).expect("unit").selected);
    }

    #[test]
    fn add_to_selection_appends_mass_selectable_unit() {
        let mut logic = two_player_logic();
        let a = spawn(&mut logic, "SelectParityUnit", 0, 0.0);
        let b = spawn(&mut logic, "SelectParityUnit", 0, 10.0);

        logic.select_objects(0, vec![a]);
        logic.select_object_list(1, vec![b], false);
        assert_eq!(
            logic.get_player(0).expect("p0").selected_objects,
            vec![a, b]
        );
    }

    #[test]
    fn select_objects_keeps_garrisoned_and_transported_members() {
        // C++ GameLogic::selectObject has no isSelectable/contained gate;
        // SelectionXlat selectDrawable recalls getLiveObjects members in bunkers.
        let mut logic = two_player_logic();
        let walker = spawn(&mut logic, "SelectParityUnit", 0, 0.0);
        let rider = spawn(&mut logic, "SelectParityUnit", 0, 10.0);
        {
            let o = logic.host_object_mut(rider).expect("rider");
            o.set_contained_by(Some(ObjectId(99)));
            // hq-90dsv: control-group recall is not this bead. Live selectObject
            // still gates is_selectable; clear UNSELECTABLE so this test stays
            // the contained/MASKED (no-contained-gate) check.
            o.set_status_unselectable(false);
            o.set_ai_state(crate::game_logic::AIState::Garrisoned);
            o.status.masked = true;
        }
        logic.select_objects(0, vec![walker, rider]);
        assert_eq!(
            logic.get_player(0).expect("p0").selected_objects,
            vec![walker, rider],
            "control-group recall must keep garrisoned/transported members"
        );
        assert!(logic.host_object(rider).expect("rider").selected);
    }

    #[test]
    fn inspect_enemy_select_is_silent() {
        // C++ CommandXlat.cpp:3502-3516 weeds to isLocallyControlled.
        bind_select_voice();
        let mut logic = two_player_logic();
        let theirs = spawn(&mut logic, "SelectParityUnit", 1, 20.0);
        logic.queued_audio_events.clear();
        logic.select_objects(0, vec![theirs]);
        assert_eq!(
            logic.get_player(0).expect("p0").selected_objects,
            vec![theirs]
        );
        assert!(
            queued_select_voices(&logic).is_empty(),
            "inspect-select enemy must not VoiceSelect: {:?}",
            logic.queued_audio_events
        );
        clear_test_template_voices();
    }

    #[test]
    fn local_select_plays_voice_select() {
        bind_select_voice();
        let mut logic = two_player_logic();
        let mine = spawn(&mut logic, "SelectParityUnit", 0, 0.0);
        logic.queued_audio_events.clear();
        logic.select_objects(0, vec![mine]);
        assert_eq!(queued_select_voices(&logic), vec![TEST_VOICE_SELECT]);
        clear_test_template_voices();
    }

    #[test]
    fn mixed_select_weeds_to_local_voice() {
        bind_select_voice();
        let mut logic = two_player_logic();
        let mine = spawn(&mut logic, "SelectParityUnit", 0, 0.0);
        let theirs = spawn(&mut logic, "SelectParityUnit", 1, 20.0);
        logic.queued_audio_events.clear();
        logic.select_objects(0, vec![theirs, mine]);
        let voices: Vec<_> = logic
            .queued_audio_events
            .iter()
            .filter(|e| e.event_type == TEST_VOICE_SELECT)
            .map(|e| e.object_id)
            .collect();
        assert_eq!(voices, vec![Some(mine)]);
        clear_test_template_voices();
    }

    #[test]
    fn select_objects_no_sound_is_silent() {
        // C++ MSG_CREATE_SELECTED_GROUP_NO_SOUND / ADD_TEAM / REMOVE_FROM_SELECTED_GROUP.
        bind_select_voice();
        let mut logic = two_player_logic();
        let mine = spawn(&mut logic, "SelectParityUnit", 0, 0.0);
        logic.queued_audio_events.clear();
        logic.select_objects_no_sound(0, vec![mine]);
        assert_eq!(
            logic.get_player(0).expect("p0").selected_objects,
            vec![mine]
        );
        assert!(
            queued_select_voices(&logic).is_empty(),
            "NO_SOUND / ADD_TEAM / deselect must not VoiceSelect: {:?}",
            logic.queued_audio_events
        );
        clear_test_template_voices();
    }

    #[test]
    fn reclick_create_selected_group_replays_voice() {
        // C++ always posts MSG_CREATE_SELECTED_GROUP on re-click.
        bind_select_voice();
        let mut logic = two_player_logic();
        let mine = spawn(&mut logic, "SelectParityUnit", 0, 0.0);
        logic.select_objects(0, vec![mine]);
        logic.queued_audio_events.clear();
        logic.queue_create_selected_group_voice(&[mine]);
        assert_eq!(queued_select_voices(&logic), vec![TEST_VOICE_SELECT]);
        logic.queued_audio_events.clear();
        logic.queue_create_selected_group_voice(&[mine]);
        assert_eq!(
            queued_select_voices(&logic),
            vec![TEST_VOICE_SELECT],
            "re-click already-selected must replay VoiceSelect"
        );
        clear_test_template_voices();
    }

    #[test]
    fn select_overlord_flashes_portable_addon() {
        use crate::game_logic::host_overlord_addons::UPGRADE_OVERLORD_GATTLING;
        use crate::game_logic::host_saboteur::SABOTEUR_FLASH_DECAY_FRAMES;

        let mut logic = two_player_logic();
        ensure_tpl(
            &mut logic,
            "ChinaTankOverlord",
            &[KindOf::Vehicle, KindOf::Selectable],
        );
        let overlord = spawn(&mut logic, "ChinaTankOverlord", 0, 0.0);
        logic.apply_upgrade_to_object(overlord, UPGRADE_OVERLORD_GATTLING);
        let addon = logic
            .host_object(overlord)
            .and_then(|o| o.overlord_portable_occupant)
            .expect("portable occupant");

        logic.select_objects(0, vec![overlord]);
        assert_eq!(
            logic
                .host_object(overlord)
                .expect("hull")
                .selection_flash_remaining,
            SABOTEUR_FLASH_DECAY_FRAMES
        );
        assert_eq!(
            logic
                .host_object(addon)
                .expect("addon")
                .selection_flash_remaining,
            SABOTEUR_FLASH_DECAY_FRAMES,
            "portable addon must flash with the hull"
        );
        assert!(
            !logic.host_object(addon).expect("addon").selected,
            "addon is flashed, not added to the selection list"
        );
        assert_eq!(
            logic.get_player(0).expect("p0").selected_objects,
            vec![overlord]
        );
    }

    #[test]
    fn select_helix_flashes_portable_addon_not_infantry() {
        use crate::game_logic::host_overlord_addons::UPGRADE_HELIX_GATTLING;
        use crate::game_logic::host_saboteur::SABOTEUR_FLASH_DECAY_FRAMES;

        let mut logic = two_player_logic();
        ensure_tpl(
            &mut logic,
            "ChinaVehicleHelix",
            &[KindOf::Vehicle, KindOf::Aircraft, KindOf::Selectable],
        );
        let helix = spawn(&mut logic, "ChinaVehicleHelix", 0, 0.0);
        logic.apply_upgrade_to_object(helix, UPGRADE_HELIX_GATTLING);
        let infantry = spawn(&mut logic, "SelectParityUnit", 0, 10.0);
        {
            let h = logic.host_object_mut(helix).expect("helix");
            if !h.occupants.contains(&infantry) {
                h.occupants.push(infantry);
            }
        }
        {
            let i = logic.host_object_mut(infantry).expect("infantry");
            i.set_contained_by(Some(helix));
        }
        let addon = logic
            .host_object(helix)
            .and_then(|o| o.overlord_portable_occupant)
            .expect("helix portable occupant");

        logic.select_objects(0, vec![helix]);
        assert_eq!(
            logic
                .host_object(helix)
                .expect("hull")
                .selection_flash_remaining,
            SABOTEUR_FLASH_DECAY_FRAMES
        );
        assert_eq!(
            logic
                .host_object(addon)
                .expect("addon")
                .selection_flash_remaining,
            SABOTEUR_FLASH_DECAY_FRAMES,
            "Helix portable addon must flash with the hull"
        );
        assert_eq!(
            logic
                .host_object(infantry)
                .expect("infantry")
                .selection_flash_remaining,
            0,
            "Helix infantry occupants must not flash"
        );
    }
}
