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
        let any_local = player_ids
            .iter()
            .any(|id| self.is_local_selection_player(*id));

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
                if any_local {
                    self.admit_local_drawable_deselected(old_id);
                }
            }
        }

        for &object_id in &accepted {
            if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.select();
                // C++ Drawable::flashAsSelected residual on select / create-team.
                obj.flash_as_selected();
            }
            if any_local {
                self.admit_local_drawable_selected(object_id);
            }
            // C++ Drawable::onSelected → contain->clientVisibleContainedFlashAsSelected.
            self.client_visible_contained_flash_as_selected(object_id);
        }

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
        self.admit_local_drawable_deselected(id);
        for player in self.players.values_mut() {
            player.selected_objects.retain(|&x| x != id);
        }
    }

    /// Drain leftover `updateHiddenStatus` onto live player selection.
    pub fn drain_hidden_drawable_selection(&mut self) {
        let hidden: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, obj)| obj.drawable_is_effectively_hidden())
            .map(|(&id, _)| id)
            .collect();
        for id in hidden {
            self.deselect_drawable(id);
        }
    }

    /// Drain leftover `maskObject` deselect onto live player selection.
    pub fn drain_masked_object_selection(&mut self) {
        // Object-local mask changes already clear the drawable bit. Admit
        // only this session's actual selection, never another world's TLS log.
        let removed: Vec<_> = self
            .selected_objects
            .iter()
            .copied()
            .filter(|id| self.objects.get(id).is_none_or(|object| !object.selected))
            .collect();
        for id in removed {
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
                .map(|p| self.is_local_player(player_id))
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
        self.apply_direct_player_order_with_effects(order, &mut Default::default())
    }

    pub(crate) fn apply_direct_player_order_with_effects(
        &mut self,
        order: DirectPlayerOrder,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        match order {
            DirectPlayerOrder::Attack { player_id, target } => {
                self.command_attack_with_effects(player_id, target, effects);
            }
            DirectPlayerOrder::Stop { player_id } => {
                self.command_stop_with_effects(player_id, effects);
            }
            DirectPlayerOrder::Move { player_id, dest } => {
                self.command_move(player_id, dest);
            }
            DirectPlayerOrder::AttackMove { player_id, dest } => {
                self.command_attack_move_with_effects(player_id, dest, effects);
            }
        }
    }

    /// Wave 931: single object-lifecycle authority boundary.
    #[inline]
    pub fn apply_object_lifecycle_op(&mut self, op: ObjectLifecycleOp) -> ObjectLifecycleResult {
        self.apply_object_lifecycle_op_with_effects(op, &mut Default::default())
    }

    pub(crate) fn apply_object_lifecycle_op_with_effects(
        &mut self,
        op: ObjectLifecycleOp,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> ObjectLifecycleResult {
        match op {
            ObjectLifecycleOp::Create {
                name,
                team,
                spawn_at,
            } => ObjectLifecycleResult::Created(
                self.create_object_with_script_effects(&name, team, spawn_at, None, effects),
            ),
            ObjectLifecycleOp::Destroy { id } => {
                self.destroy_object_with_effects(id, effects);
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
            } => ObjectLifecycleResult::Bool(self.enqueue_production_with_effects(
                producer,
                template_name,
                effects,
            )),
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
        self.apply_command_pipeline_op_with_effects(op, &mut Default::default())
    }

    pub(crate) fn apply_command_pipeline_op_with_effects(
        &mut self,
        op: CommandPipelineOp,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        match op {
            CommandPipelineOp::Queue { command } => {
                self.queue_command(command);
                false
            }
            CommandPipelineOp::QueueAndProcess { command } => {
                self.queue_and_process_command_with_effects(command, ui)
            }
            CommandPipelineOp::ProcessIfNeeded => self.process_commands_if_needed_with_effects(ui),
        }
    }

    /// Wave 933: single session-control authority boundary.
    #[inline]
    pub fn apply_session_control_op(&mut self, op: SessionControlOp) {
        self.apply_session_control_op_with_effects(op, &mut Default::default())
    }

    pub(crate) fn apply_session_control_op_with_effects(
        &mut self,
        op: SessionControlOp,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
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
            SessionControlOp::StartNewGameInCampaignSession {
                mode,
                player_id,
                faction_team,
                player_template,
                setup_skirmish_ai,
                is_campaign,
                is_challenge,
            } => {
                self.start_new_game_in_campaign_session(mode, is_campaign, is_challenge);
                if let Some(identity) = player_template {
                    if !self.bind_player_template_identity(player_id, identity) {
                        let removed_local = self.local_player_id() == Some(player_id);
                        self.players.remove(&player_id);
                        if removed_local {
                            self.select_local_player(None);
                        }
                        self.player_template_bindings.remove(&player_id);
                    }
                } else {
                    let _ = self.set_player_team(player_id, faction_team);
                }
                if setup_skirmish_ai {
                    self.setup_skirmish_ai(player_id);
                }
            }
            SessionControlOp::Reset => {
                self.reset();
            }
            SessionControlOp::OverrideWorldSize { width, height } => {
                self.override_world_size_with_effects(width, height, effects);
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
    ) -> gamelogic::GameLogicResult<ProductionAuthorityResult> {
        self.apply_production_authority_op_with_effects(op, &mut Default::default())
    }

    pub(crate) fn apply_production_authority_op_with_effects(
        &mut self,
        op: ProductionAuthorityOp,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<ProductionAuthorityResult> {
        match op {
            ProductionAuthorityOp::SpawnUnit {
                template,
                team,
                owner_player_id,
                spawn_pos,
            } => {
                let team = match owner_player_id {
                    Some(owner) => match self.players.get(&owner) {
                        Some(player) => player.team,
                        None => return Ok(ProductionAuthorityResult::Spawned(None)),
                    },
                    None => team,
                };
                Ok(ProductionAuthorityResult::Spawned(
                    self.host_spawn_production_unit_with_effects(
                        &template,
                        team,
                        owner_player_id,
                        spawn_pos,
                        ui,
                    ),
                ))
            }
            ProductionAuthorityOp::ApplySpawnReadyCompletions => {
                self.host_apply_production_spawn_ready_completions_with_effects(ui)?;
                Ok(ProductionAuthorityResult::Unit)
            }
        }
    }

    /// Wave 940: host ObjectId create/mark-destroy authority boundary.
    #[inline]
    pub fn apply_host_object_id_op(&mut self, op: HostObjectIdOp) -> HostObjectIdResult {
        self.apply_host_object_id_op_with_effects(op, &mut Default::default())
    }

    pub(crate) fn apply_host_object_id_op_with_effects(
        &mut self,
        op: HostObjectIdOp,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> HostObjectIdResult {
        match op {
            HostObjectIdOp::MarkForDestruction { id, team } => {
                self.mark_object_for_destruction_with_effects(id, team, effects);
                HostObjectIdResult::Unit
            }
            HostObjectIdOp::Create {
                template,
                team,
                spawn_at,
            } => HostObjectIdResult::Created(
                self.create_object_with_script_effects(&template, team, spawn_at, None, effects),
            ),
        }
    }

    /// Wave 941/942: host residual mutation authority boundary.
    #[inline]
    pub fn apply_host_residual_mutation_op(&mut self, op: HostResidualMutationOp) {
        self.apply_host_residual_mutation_op_with_effects(op, &mut Default::default())
    }

    pub(crate) fn apply_host_residual_mutation_op_with_effects(
        &mut self,
        op: HostResidualMutationOp,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let frame = self.frame;
        match op {
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
                if let Some(pos) = position {
                    if !self.set_owned_object_position_with_effects(id, pos, frame, effects) {
                        return;
                    }
                }
                if let Some(obj) = self.objects.get_mut(&id) {
                    {
                        obj.internal_change_health(-obj.health.current);
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
                    self.apply_host_object_id_op_with_effects(
                        HostObjectIdOp::MarkForDestruction { id, team },
                        effects,
                    );
                }
            }
            HostResidualMutationOp::DestroyBomb { id, mark_destroy } => {
                if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                    obj.internal_change_health(-obj.health.current);
                    obj.status.destroyed = true;
                }
                if mark_destroy {
                    self.mark_object_for_destruction_with_effects(id, None, effects);
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
                let draw_icon_ui = self.get_draw_icon_ui();
                if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                    let was = obj.weapon_bonus_horde;
                    obj.weapon_bonus_horde = now_horde;
                    obj.apply_horde_terrain_decal(was, now_horde, draw_icon_ui);

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
            HostResidualMutationOp::SetPosition {
                id,
                position,
                sticky_follow_tick,
            } => {
                self.set_owned_object_position_with_effects(id, position, frame, effects);
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
        }
    }

    /// Wave 943: apply post-armor damage for host objects with no shadow mapping.
    /// Returns number of host objects mutated.

    /// Wave 944: apply one shadow→host writeback mutation.

    /// Borrow the canonical object for one synchronous mutation.
    /// Observation sessions cannot replace its fields or receive implicit writes.
    pub fn with_host_object_mut<R>(
        &mut self,
        id: ObjectId,
        f: impl FnOnce(&mut crate::game_logic::object::Object) -> R,
    ) -> Option<R> {
        let result = self.objects.get_mut(&id).map(f);
        self.admit_object_drawable_deselection(id);
        result
    }

    /// Current ammo on the receiving world's canonical primary weapon.
    pub fn host_authoritative_weapon_ammo(&self, id: ObjectId) -> Option<u32> {
        self.host_object(id)
            .and_then(|o| o.weapon.as_ref())
            .and_then(|w| w.ammo)
    }

    /// Freeze the C++ `Drawable::updateDrawableClipStatus` arguments for all
    /// concrete host WeaponSet slots.
    ///
    /// Read each owned WeaponSet slot in C++ iteration order. Presentation
    /// freezes these facts without selecting a separate entity store.
    pub fn host_authoritative_projectile_clip_statuses(
        &self,
        id: ObjectId,
    ) -> [Option<(u32, u32)>; 3] {
        let valid_clip = |shots_remaining: u32, max_shots: u32| {
            (max_shots > 0 && shots_remaining <= max_shots).then_some((shots_remaining, max_shots))
        };

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
        obj.set_attack_substate(sub);
        obj.set_approach_timestamp(frame);
        Some(frame)
    }

    /// AttackStateMachine substate on the receiving object.
    pub fn host_authoritative_attack_substate(&self, id: ObjectId) -> Option<u8> {
        self.host_object(id)
            .map(|o| o.attack_substate().to_ordinal())
    }

    /// Occupant/garrison count on the receiving world.
    /// TunnelContain redirects to the controlling player's TunnelTracker.
    pub fn host_authoritative_occupant_count(&self, id: ObjectId) -> Option<u16> {
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
        let count = o
            .building_data
            .as_ref()
            .map_or(o.occupants.len(), |building| {
                building.garrisoned_units.len()
            });
        Some(count.min(u16::MAX as usize) as u16)
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

    /// Current move destination; None means the receiving object is stopped.
    pub fn host_authoritative_move_dest(&self, id: ObjectId) -> Option<[f32; 3]> {
        self.host_object(id)
            .and_then(|o| o.movement.target_position)
            .map(|p| p.to_array())
    }

    /// Current health of the receiving world's canonical body at every phase.
    pub fn host_authoritative_health(&self, id: ObjectId) -> Option<f32> {
        self.host_object(id).map(|o| o.health.current)
    }

    /// Authoritative construction: `(percent, under_construction)`.
    /// Fraction is 0–1; use `host_authoritative_construction_cpp` for 0–100.
    pub fn host_authoritative_construction(&self, id: ObjectId) -> Option<(f32, bool)> {
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

    /// Pose of the receiving world's canonical object.
    pub fn host_authoritative_pose(&self, id: ObjectId) -> Option<[f32; 3]> {
        self.host_object(id).map(|o| o.get_position().to_array())
    }

    /// Cash owned by the receiving player.
    pub fn host_authoritative_cash(&self, player_id: u32) -> Option<u32> {
        self.get_player(player_id).map(|p| p.resources.supplies)
    }

    /// Target of the receiving object; None is an actual cleared target.
    pub fn host_authoritative_target(&self, id: ObjectId) -> Option<ObjectId> {
        self.host_object(id).and_then(|o| o.target)
    }

    /// Wave 955/958: host-authority object borrow (preferred over get_object dual-read).
    /// The world owns these live values. A comparison session is an observer.
    #[inline]
    pub fn host_object(&self, id: ObjectId) -> Option<&crate::game_logic::object::Object> {
        self.objects.lookup(&id)
    }

    /// Wave 955/958: host-authority object map borrow (command apply / AI / shadow).
    /// Presentation dual-read paths must use `PresentationFrame`, not this.
    #[inline]
    pub fn host_objects(&self) -> crate::game_logic::HostObjectRead<'_> {
        self.objects.read()
    }

    pub(crate) fn has_unregistered_host_objects(&self) -> bool {
        self.objects.has_unregistered_lookup()
    }

    /// Wave 955/958: host-authority mutable object map borrow.
    #[inline]
    pub fn host_objects_mut(&mut self) -> &mut HostObjectStore {
        &mut self.objects
    }

    /// Wave 950/958: host-authority mutable object borrow.
    #[inline]
    pub fn host_object_mut(
        &mut self,
        id: ObjectId,
    ) -> Option<&mut crate::game_logic::object::Object> {
        self.objects.lookup_mut(&id)
    }

    /// Internal stable storage access for an Object lifecycle callback.
    /// During C++-ordered reset teardown, ordinary ID lookup is empty while
    /// the still-linked Object values remain available to their destructors.
    pub(in crate::game_logic) fn teardown_object(
        &self,
        id: ObjectId,
    ) -> Option<&crate::game_logic::object::Object> {
        self.objects.get(&id)
    }

    pub(in crate::game_logic) fn teardown_object_mut(
        &mut self,
        id: ObjectId,
    ) -> Option<&mut crate::game_logic::object::Object> {
        self.objects.get_mut(&id)
    }

    #[cfg(test)]
    pub(crate) fn set_fixture_object_position(&mut self, id: ObjectId, position: Vec3) {
        let frame = self.frame;
        let (object, partition) = self
            .host_object_and_partition_mut(id)
            .expect("fixture Object");
        object.set_position_with_partition_owner(
            position,
            frame,
            &mut Default::default(),
            partition,
        );
    }

    #[cfg(test)]
    pub(crate) fn set_fixture_object_orientation(&mut self, id: ObjectId, angle: f32) {
        let (object, partition) = self
            .host_object_and_partition_mut(id)
            .expect("fixture Object");
        object.set_orientation_with_partition_owner(angle, partition);
    }

    #[cfg(test)]
    pub(crate) fn set_fixture_object_contained_by(
        &mut self,
        id: ObjectId,
        container: Option<ObjectId>,
    ) {
        let (object, partition) = self
            .host_object_and_partition_mut(id)
            .expect("fixture Object");
        object.set_contained_by_with_partition_owner(container, true, partition);
    }

    #[cfg(test)]
    pub(crate) fn set_fixture_object_geometry(
        &mut self,
        id: ObjectId,
        geometry: crate::game_logic::GeometryInfo,
    ) {
        let (object, partition) = self
            .host_object_and_partition_mut(id)
            .expect("fixture Object");
        object.set_geometry_info_with_partition_owner(geometry, partition);
    }

    /// CommandExecutor's non-AI fallback still belongs to this Object owner.
    /// Complete any aircraft takeoff transform before the caller observes success.
    pub(crate) fn set_owned_fallback_attack_target_with_effects(
        &mut self,
        id: ObjectId,
        target: ObjectId,
        forced: bool,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let _ = self.host_object_mut(id);
        let Some(object) = self.objects.get_mut(&id) else {
            return false;
        };
        if forced {
            object.set_force_attack(true);
        }
        object.set_target_with_operation(
            Some(target),
            &mut self.attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            self.frame,
            &mut effects.reborrow_for_object(&mut self.partition_manager),
        );
        true
    }

    /// Split command services and the simulation owner for nested aircraft moves.
    pub(crate) fn host_object_attack_audio_partition_mut(
        &mut self,
        id: ObjectId,
    ) -> Option<(
        &mut Object,
        &mut crate::game_logic::host_attack_log::AttackEvents,
        &mut crate::game_logic::SessionAudio,
        &mut crate::game_logic::partition_manager::PartitionManager,
    )> {
        let _ = self.host_object_mut(id)?;
        Some((
            self.objects.get_mut(&id)?,
            &mut self.attack_events,
            &mut self.queued_audio_events,
            &mut self.partition_manager,
        ))
    }

    pub(crate) fn host_object_movement_audio_partition_mut(
        &mut self,
        id: ObjectId,
    ) -> Option<(
        &mut Object,
        crate::game_logic::movement_events::MovementEventWriter<'_>,
        &mut crate::game_logic::SessionAudio,
        &mut crate::game_logic::partition_manager::PartitionManager,
    )> {
        let _ = self.host_object_mut(id)?;
        Some((
            self.objects.get_mut(&id)?,
            self.movement_events.writer(self.frame),
            &mut self.queued_audio_events,
            &mut self.partition_manager,
        ))
    }

    /// A short explicit loan for Object operations that change PartitionData.
    /// Preserve host_object_mut's named-unit observation before splitting fields.
    pub(crate) fn host_object_and_partition_mut(
        &mut self,
        id: ObjectId,
    ) -> Option<(
        &mut Object,
        &mut crate::game_logic::partition_manager::PartitionManager,
    )> {
        let _ = self.host_object_mut(id)?;
        Some((self.objects.get_mut(&id)?, &mut self.partition_manager))
    }

    /// Same named-unit observation as host_objects_mut, with a disjoint owner loan.
    pub(crate) fn host_objects_and_partition_mut(
        &mut self,
    ) -> (
        &mut HostObjectStore,
        &mut crate::game_logic::partition_manager::PartitionManager,
    ) {
        let _ = self.host_objects_mut();
        (&mut self.objects, &mut self.partition_manager)
    }

    /// Split the canonical Object and this session's existing audio output.
    /// Preserve the same named-unit observation as host_object_mut.
    pub(crate) fn host_object_and_audio_events_mut(
        &mut self,
        id: ObjectId,
    ) -> Option<(
        &mut crate::game_logic::object::Object,
        &mut crate::game_logic::SessionAudio,
    )> {
        self.objects
            .get_mut(&id)
            .map(|object| (object, &mut self.queued_audio_events))
    }

    /// Borrow the canonical subject and its receiving match's presentation
    /// output together. Preserve named-unit history exactly as host_object_mut.
    pub(crate) fn host_object_attack_audio_mut(
        &mut self,
        id: ObjectId,
    ) -> Option<(
        &mut crate::game_logic::Object,
        &mut crate::game_logic::host_attack_log::AttackEvents,
        &mut crate::game_logic::SessionAudio,
    )> {
        let _ = self.host_object_mut(id)?;
        Some((
            self.objects.get_mut(&id)?,
            &mut self.attack_events,
            &mut self.queued_audio_events,
        ))
    }

    pub(crate) fn host_object_movement_audio_mut(
        &mut self,
        id: ObjectId,
    ) -> Option<(
        &mut crate::game_logic::Object,
        crate::game_logic::movement_events::MovementEventWriter<'_>,
        &mut crate::game_logic::SessionAudio,
    )> {
        let _ = self.host_object_mut(id)?;
        Some((
            self.objects.get_mut(&id)?,
            self.movement_events.writer(self.frame),
            &mut self.queued_audio_events,
        ))
    }

    pub(crate) fn host_object_and_attack_events_mut(
        &mut self,
        id: ObjectId,
    ) -> Option<(
        &mut crate::game_logic::object::Object,
        &mut crate::game_logic::host_attack_log::AttackEvents,
    )> {
        self.objects
            .get_mut(&id)
            .map(|object| (object, &mut self.attack_events))
    }

    pub(crate) fn host_object_ownership_partition_mut(
        &mut self,
        id: ObjectId,
    ) -> Option<(
        &mut Object,
        &mut crate::game_logic::host_owner_log::OwnershipEvents,
        &mut crate::game_logic::partition_manager::PartitionManager,
    )> {
        self.objects.get_mut(&id).map(|object| {
            (
                object,
                &mut self.ownership_events,
                &mut self.partition_manager,
            )
        })
    }
    pub(crate) fn host_object_health_partition_mut(
        &mut self,
        id: ObjectId,
    ) -> Option<(
        &mut Object,
        &mut crate::game_logic::HostHealthEvents,
        &mut crate::game_logic::partition_manager::PartitionManager,
    )> {
        self.objects
            .get_mut(&id)
            .map(|object| (object, &mut self.health_events, &mut self.partition_manager))
    }

    pub(crate) fn host_object_and_ownership_events_mut(
        &mut self,
        id: ObjectId,
    ) -> Option<(
        &mut crate::game_logic::object::Object,
        &mut crate::game_logic::host_owner_log::OwnershipEvents,
    )> {
        self.objects
            .get_mut(&id)
            .map(|object| (object, &mut self.ownership_events))
    }

    pub(crate) fn set_host_object_name(&mut self, id: ObjectId, name: &str) -> bool {
        if name.is_empty() {
            return false;
        }
        let Some(object) = self.objects.get_mut(&id) else {
            return false;
        };
        object.name = name.to_owned();

        true
    }

    /// Set an admitted object's immediate movement goal and record the change
    /// in this world's output. Existing velocity and path state are retained.
    pub fn move_object_to(&mut self, id: ObjectId, position: glam::Vec3) -> bool {
        let frame = self.frame;
        let Some((object, mut movement_events, audio_events)) =
            self.host_object_movement_audio_mut(id)
        else {
            return false;
        };
        if !object.is_mobile() || !object.is_alive() {
            return false;
        }
        object.move_to_at_frame(
            position,
            frame,
            &mut movement_events,
            &mut crate::game_logic::ObjectAudio::queue(audio_events),
        );
        true
    }

    /// Borrow the admitted object and its world's ordered health transport.
    /// Split the owner borrow without selecting another world or copying fields.
    pub(crate) fn host_object_and_health_events_mut(
        &mut self,
        id: ObjectId,
    ) -> Option<(
        &mut crate::game_logic::object::Object,
        &mut crate::game_logic::HostHealthEvents,
    )> {
        self.objects
            .get_mut(&id)
            .map(|object| (object, &mut self.health_events))
    }

    /// Borrow an admitted object and a bounded recorder from the same owner.
    /// No object backlink or ambient selection participates in this operation.
    pub(crate) fn host_object_and_movement_events_mut(
        &mut self,
        id: ObjectId,
    ) -> Option<(
        &mut crate::game_logic::object::Object,
        crate::game_logic::movement_events::MovementEventWriter<'_>,
    )> {
        self.objects
            .get_mut(&id)
            .map(|object| (object, self.movement_events.writer(self.frame)))
    }

    /// Issue attack command to selected objects
    pub fn command_attack(&mut self, player_id: u32, target_id: ObjectId) {
        self.command_attack_with_effects(player_id, target_id, &mut Default::default())
    }

    pub(crate) fn command_attack_with_effects(
        &mut self,
        player_id: u32,
        target_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let frame = self.frame;
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
            let voice = self.attack_voice_slot(&selected, Some(target_id), false, false, None);
            let mut accepted_attackers = Vec::new();
            for &object_id in &selected {
                if self.flight_deck_ai_do_command_with_effects(
                    object_id,
                    crate::game_logic::host_flight_deck::HostFlightDeckCommand::AttackObject,
                    Some(target_id),
                    None,
                    effects,
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
                    obj_mut.attack_target_with_operation(
                        target_id,
                        &mut self.attack_events,
                        &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                        self.frame,
                        &mut effects.reborrow_for_object(&mut self.partition_manager),
                    );
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
                            let under =
                                crate::game_logic::weapon_bootstrap::PATHFIND_CELL_SIZE * 0.25;
                            let range = attacker
                                .selected_weapon_slot()
                                .and_then(|slot| attacker.weapon_slot(slot).map(|w| (slot, w)))
                                .map(|(slot, w)| {
                                    (attacker.effective_weapon_range_for_slot_with_solo_set(
                                        slot,
                                        w.range,
                                        &self.solo_weapon_bonus_set,
                                    ) - under)
                                        .max(0.0)
                                })
                                .unwrap_or(50.0);
                            let from = attacker.get_position();
                            let mut dir = tpos - from;
                            dir.y = 0.0;
                            let dist = dir.length();
                            if dist > range * 0.8 {
                                // Movement authority: no range-snap teleport. Path was
                                // already issued via assign_unit_attack_path; GameWorld
                                // integrates the march. Host-only residual may still snap
                                // for short smoke waits when authority is off.
                                {
                                    let dir = if dist > 1.0 {
                                        dir / dist
                                    } else {
                                        glam::Vec3::new(1.0, 0.0, 0.0)
                                    };
                                    let stand = tpos - dir * (range * 0.55);
                                    let stand = glam::Vec3::new(stand.x, from.y, stand.z);
                                    if let Some(a) = self.objects.get_mut(&object_id) {
                                        a.set_position_with_partition_owner(
                                            stand,
                                            frame,
                                            effects,
                                            &mut self.partition_manager,
                                        );
                                        a.attack_target_with_operation(
                                            target_id,
                                            &mut self.attack_events,
                                            &mut crate::game_logic::ObjectAudio::queue(
                                                &mut self.queued_audio_events,
                                            ),
                                            self.frame,
                                            &mut effects
                                                .reborrow_for_object(&mut self.partition_manager),
                                        );
                                        // Host-immediate engagement residual (host-only
                                        // path when movement auth is off).
                                        a.set_ai_state(
                                            AIState::Attacking,
                                            &mut crate::game_logic::ObjectAudio::queue(
                                                &mut self.queued_audio_events,
                                            ),
                                        );
                                        {}
                                        a.set_status_attacking(true);
                                        a.set_status_moving(false);
                                        a.movement.velocity = glam::Vec3::ZERO;

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
                .map(|p| self.is_local_player(player_id))
                .unwrap_or(false);
            if local {
                self.queue_picked_unit_voice(&accepted_attackers, voice);
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
    /// CommandXlat observes the current weapon while translating the input,
    /// before the logic command may choose a new weapon. This pure semantic
    /// observation consumes no voice-picking RNG and publishes no event.
    pub(crate) fn attack_voice_slot(
        &self,
        unit_ids: &[ObjectId],
        target_id: Option<ObjectId>,
        specialty_weapon: bool,
        at_location: bool,
        forced_slot: Option<u8>,
    ) -> crate::game_logic::audio_dispatch_impl::UnitVoiceSlot {
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
        pick_specialty_attack_voice(default, weapons, structure, specialty_weapon, at_location)
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
    /// Missing VoiceMoveUpgraded is silent but still suppresses default VoiceMove.
    pub fn try_queue_picked_voice_move_upgraded(&mut self, unit_ids: &[ObjectId]) -> bool {
        use crate::game_logic::audio_dispatch_impl::resolve_admitted_per_unit_sound;
        use crate::game_logic::host_gla_worker::{
            WORKER_VOICE_MOVE_UPGRADED, is_worker_for_voice_move_upgraded,
            worker_shoes_voice_upgrade_complete,
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
            ) {
                continue;
            }
            let player_names = |p: &crate::game_logic::Player| {
                worker_shoes_voice_upgrade_complete(p.completed_upgrades.iter())
            };
            let player_has_shoes = obj
                .owner_player_id
                .and_then(|pid| self.players.get(&pid))
                .map(player_names)
                .unwrap_or(false);
            if !player_has_shoes {
                continue;
            }
            if let Some(event) =
                resolve_admitted_per_unit_sound(obj.get_template(), WORKER_VOICE_MOVE_UPGRADED)
            {
                let pos = obj.get_position();
                self.queue_audio_event(
                    AudioEventRequest::new(&event)
                        .with_object(id)
                        .with_position(pos)
                        .with_priority(100),
                );
            }
            // C++ sets `skip = true` for the upgraded worker even when the
            // per-unit cue is absent; the final null-sound check is silent and
            // suppresses the ordinary VoiceMove fallback.
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
            resolve_receiving_unit_voice, resolve_terrorist_in_car_voice,
        };
        let mut chosen: Option<(String, ObjectId, glam::Vec3, bool)> = None;
        for &id in unit_ids {
            let Some(obj) = self.objects.get(&id) else {
                continue;
            };
            if obj.is_kind_of(KindOf::IgnoredInGui) {
                continue;
            }
            let Some(event) = resolve_receiving_unit_voice(obj.get_template(), slot) else {
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
        // C++ containers carry a ContainModuleInterface (Humvee = OpenContain on
        // KINDOF_VEHICLE); plain infantry have none (Object::isHero walks
        // getContain()), so the container fixture must be a vehicle — matches
        // radar_live host_radar_contained_hero_marks_container.
        ensure_tpl(
            &mut logic,
            "SelectParityTransport",
            &[KindOf::Vehicle, KindOf::Selectable],
        );
        let humvee = spawn(&mut logic, "SelectParityTransport", 0, 0.0);
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
            let (o, partition) = logic.host_object_and_partition_mut(rider).expect("rider");
            o.set_contained_by_with_partition_owner(Some(ObjectId(99)), true, partition);
            // hq-90dsv: control-group recall is not this bead. Live selectObject
            // still gates is_selectable; clear UNSELECTABLE so this test stays
            // the contained/MASKED (no-contained-gate) check.
            o.set_status_unselectable(false);
            o.set_ai_state(
                crate::game_logic::AIState::Garrisoned,
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            );
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
            let (i, partition) = logic
                .host_object_and_partition_mut(infantry)
                .expect("infantry");
            i.set_contained_by_with_partition_owner(Some(helix), true, partition);
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
