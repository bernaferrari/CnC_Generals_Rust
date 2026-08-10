//! Host objects `impl GameLogic` — `host_ops_writeback`.
//! commands, host unmapped damage, host_object, writeback. Child of `world_objects` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    /// Select objects for a player
    pub fn select_objects(&mut self, player_id: u32, object_ids: Vec<ObjectId>) {
        let Some(player_team) = self.players.get(&player_id).map(|p| p.team) else {
            return;
        };
        let is_local = self
            .players
            .get(&player_id)
            .map(|p| p.is_local)
            .unwrap_or(false);

        // Snapshot previous selection for deselect residual.
        let previous: Vec<ObjectId> = self
            .players
            .get(&player_id)
            .map(|p| p.selected_objects.clone())
            .unwrap_or_default();
        for &old_id in &previous {
            if let Some(obj) = self.objects.get_mut(&old_id) {
                obj.deselect();
            }
        }

        let mut selected = Vec::new();
        let mut voice_pos = None;
        let mut voice_template = None;
        for &object_id in &object_ids {
            if let Some(obj) = self.objects.get_mut(&object_id) {
                if obj.team == player_team && obj.is_selectable() {
                    obj.select();
                    // C++ Drawable::flashAsSelected residual on select / create-team.
                    obj.flash_as_selected();
                    selected.push(object_id);
                    if voice_pos.is_none() {
                        voice_pos = Some(obj.get_position());
                        voice_template = Some(obj.template_name.clone());
                    }
                }
            }
        }

        if let Some(player) = self.players.get_mut(&player_id) {
            player.selected_objects = selected.clone();
        }

        // C++ VoiceSelect residual (primary selection unit).
        if is_local {
            if let (Some(pos), Some(template)) = (voice_pos, voice_template) {
                let event = format!("{template}VoiceSelect");
                self.queue_audio_event(
                    AudioEventRequest::new(&event)
                        .with_position(pos)
                        .with_priority(100),
                );
                self.queue_audio_event(
                    AudioEventRequest::new("UnitVoiceSelect")
                        .with_position(pos)
                        .with_priority(90),
                );
            }
        }

        log::debug!("{} selected {} objects", player_id, selected.len());
    }

    /// Issue move command to selected objects (with pathfinding)
    pub fn command_move(&mut self, player_id: u32, target_position: Vec3) {
        if let Some(player) = self.players.get(&player_id) {
            let selected = player.selected_objects.clone();
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
            // C++ VoiceMove residual for local player.
            let local = self
                .players
                .get(&player_id)
                .map(|p| p.is_local)
                .unwrap_or(false);
            if local {
                if let Some(&oid) = selected.first() {
                    if let Some(obj) = self.objects.get(&oid) {
                        let event = format!("{}VoiceMove", obj.template_name);
                        let pos = obj.get_position();
                        self.queue_audio_event(
                            AudioEventRequest::new(&event)
                                .with_position(pos)
                                .with_priority(100),
                        );
                        self.queue_audio_event(
                            AudioEventRequest::new("UnitVoiceMove")
                                .with_position(pos)
                                .with_priority(90),
                        );
                    }
                }
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
                spawn_pos,
            } => ProductionAuthorityResult::Spawned(
                self.host_spawn_production_unit(&template, team, spawn_pos),
            ),
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
                    let _ = obj.take_damage_from_typed_death(
                        amount,
                        None,
                        crate::game_logic::combat::DamageType::Unresistable,
                        death_type,
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
                            obj.movement.velocity = glam::Vec3::new(0.0, -20.0, 0.0);
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
            } => {
                let Some(obj) = self.host_objects_mut().get_mut(&id) else {
                    return false;
                };
                obj.team = team;
                obj.team_color = team_color;
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
    pub fn with_host_object_mut<R>(
        &mut self,
        id: ObjectId,
        f: impl FnOnce(&mut crate::game_logic::object::Object) -> R,
    ) -> Option<R> {
        let obj = self.host_objects_mut().get_mut(&id)?;
        Some(f(obj))
    }

    /// Wave 955/958: host-authority object borrow (preferred over get_object dual-read).
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
        &self.objects
    }

    /// Wave 955/958: host-authority mutable object map borrow.
    #[inline]
    pub fn host_objects_mut(
        &mut self,
    ) -> &mut std::collections::HashMap<ObjectId, crate::game_logic::object::Object> {
        &mut self.objects
    }

    /// Wave 950/958: host-authority mutable object borrow.
    #[inline]
    pub fn host_object_mut(
        &mut self,
        id: ObjectId,
    ) -> Option<&mut crate::game_logic::object::Object> {
        self.objects.get_mut(&id)
    }

    /// Issue attack command to selected objects
    pub fn command_attack(&mut self, player_id: u32, target_id: ObjectId) {
        if let Some(player) = self.players.get(&player_id) {
            let Some(target_team) = self.objects.get(&target_id).map(|target| target.team) else {
                return;
            };
            if target_team == player.team {
                return;
            }

            let selected = player.selected_objects.clone();
            for &object_id in &selected {
                let can = self
                    .objects
                    .get(&object_id)
                    .is_some_and(|obj| obj.can_attack() && obj.team != target_team);
                if !can {
                    continue;
                }

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
            // C++ VoiceAttack residual for local player.
            let local = self
                .players
                .get(&player_id)
                .map(|p| p.is_local)
                .unwrap_or(false);
            if local {
                if let Some(&oid) = selected.first() {
                    if let Some(obj) = self.objects.get(&oid) {
                        let event = format!("{}VoiceAttack", obj.template_name);
                        let pos = obj.get_position();
                        self.queue_audio_event(
                            AudioEventRequest::new(&event)
                                .with_position(pos)
                                .with_priority(100),
                        );
                        self.queue_audio_event(
                            AudioEventRequest::new("UnitVoiceAttack")
                                .with_position(pos)
                                .with_priority(90),
                        );
                    }
                }
            }
            log::trace!(
                "{} commanded {} units to attack object {}",
                player_id,
                selected.len(),
                target_id
            );
        }
    }
}
