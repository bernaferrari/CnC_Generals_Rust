use super::*;

impl Object {
    /// C++ KINDOF_CAN_SURRENDER residual (infantry primarily).
    pub fn can_surrender_from_damage(&self) -> bool {
        if self.is_kind_of(crate::game_logic::KindOf::Infantry) {
            return true;
        }
        let n = self.template_name.to_ascii_lowercase();
        n.contains("infantry")
            || n.contains("ranger")
            || n.contains("rebel")
            || n.contains("redguard")
            || n.contains("tankhunter")
            || n.contains("pathfinder")
            || n.contains("colonel")
            || n.contains("jarmen")
            || n.contains("hijacker")
            || n.contains("worker")
            || n.contains("pilot")
    }

    /// Consume pending DAMAGE_DEPLOY assault signal (GameLogic combat path).
    pub fn take_pending_deploy_assault(&mut self) -> bool {
        let v = self.status.pending_deploy_assault;
        self.status.pending_deploy_assault = false;
        v
    }

    /// Consume pending DAMAGE_KILL_GARRISONED occupant kill count.
    pub fn take_pending_kill_garrisoned(&mut self) -> u32 {
        let v = self.status.pending_kill_garrisoned;
        self.status.pending_kill_garrisoned = 0;
        v
    }

    /// Residual mine / demo-trap / booby identity for DAMAGE_DISARM targeting.
    pub fn is_disarmable_mine(&self) -> bool {
        use crate::game_logic::host_mines::can_clear_mine_kind;
        if let Some(md) = self.mine_data.as_ref() {
            return !md.detonated && can_clear_mine_kind(md.kind);
        }
        // Name peel residual when mine_data not attached yet.
        let n = self.template_name.to_ascii_lowercase();
        n.contains("mine")
            || n.contains("demotrap")
            || n.contains("booby")
            || self.status.booby_trapped
    }

    /// C++ LandMineInterface::disarm residual (safe clear, no splash).
    pub fn disarm_mine_safe(&mut self) -> bool {
        if !self.is_disarmable_mine() {
            return false;
        }
        if let Some(md) = self.mine_data.as_mut() {
            md.detonated = true;
            md.proximity_enabled = false;
            md.detonate_at_frame = None;
        }
        self.health.current = 0.0;
        self.status.destroyed = true;
        true
    }

    /// C++ ActiveBody::isSubdued residual (`currentSubdual >= maxHealth`).
    #[inline]
    pub fn is_subdued(&self) -> bool {
        self.health.maximum > 0.0 && self.subdual_damage + 1e-3 >= self.health.maximum
    }

    /// C++ ActiveBody::internalAddSubdualDamage + onSubdualChange residual.
    pub fn apply_subdual_damage(&mut self, amount: f32) {
        if amount <= 0.0 || !amount.is_finite() {
            return;
        }
        // Infantry residual: subdual rarely applies (microwave targets vehicles/structures).
        if self.is_kind_of(crate::game_logic::KindOf::Infantry) {
            return;
        }
        let was = self.is_subdued();
        let cap = self.health.maximum.max(1.0);
        self.subdual_damage = (self.subdual_damage + amount).min(cap);
        // Default heal rate residual when first hit (retail-ish 30f / 5 dmg step peel).
        if self.subdual_heal_rate_frames == 0 {
            self.subdual_heal_rate_frames = 30;
            self.subdual_heal_amount = 5.0;
        }
        self.subdual_heal_countdown = self.subdual_heal_rate_frames;
        let now = self.is_subdued();
        if now != was {
            self.set_disabled_subdued(now);
        } else if now {
            self.set_disabled_subdued(true);
        }
    }

    /// C++ SubdualDamageHelper::update residual heal step.
    pub fn tick_subdual_damage(&mut self) {
        if self.subdual_damage <= 0.0 {
            if self.status.disabled_subdued && !self.is_emp_disabled() {
                // Keep subdued clear if no other disable source.
                // Only clear subdual-driven disable when subdual healed out.
            }
            return;
        }
        if self.subdual_heal_rate_frames == 0 || self.subdual_heal_amount <= 0.0 {
            return;
        }
        if self.subdual_heal_countdown > 0 {
            self.subdual_heal_countdown -= 1;
            return;
        }
        let was = self.is_subdued();
        self.subdual_damage = (self.subdual_damage - self.subdual_heal_amount).max(0.0);
        self.subdual_heal_countdown = self.subdual_heal_rate_frames;
        let now = self.is_subdued();
        if was && !now {
            self.set_disabled_subdued(false);
        }
    }

    pub fn set_disabled_subdued(&mut self, subdued: bool) {
        if subdued {
            self.set_status_disabled_subdued(true);
            // C++ orderAllPassengersToIdle residual: drop attack / move orders.
            self.status.attacking = false;
            self.set_status_force_attack(false);
            self.target = None;
            self.target_location = None;
            // Structures do not move; stop any residual production-related AI.
            if !self.is_kind_of(KindOf::Structure) {
                self.set_status_moving(false);
                self.stop_moving();
                self.set_ai_state(AIState::Idle);
            }
        } else {
            self.set_status_disabled_subdued(false);
        }
    }

    /// Apply kill-pilot residual: vehicle becomes unmanned (no HP change).
    /// Caller is responsible for team transfer (typically Neutral).
    /// Captures `unmanned_owner_team` for PilotFindVehicle PartitionFilterPlayer residual.
    pub fn apply_kill_pilot_unmanned(&mut self) {
        // Preserve original controller for same-player PartitionFilter residual.
        // Only snapshot on the edge into unmanned (refresh would overwrite Neutral).
        if !self.status.disabled_unmanned {
            self.status.unmanned_owner_team = Some(self.team);
        }
        self.set_status_disabled_unmanned(true);
        self.set_status_disabled_hacked(false);
        self.status.disabled_hacked_until_frame = 0;
        self.set_status_disabled_emp(false);
        self.status.disabled_emp_until_frame = 0;
        self.set_status_disabled_paralyzed(false);
        self.status.disabled_paralyzed_until_frame = 0;
        self.status.attacking = false;
        self.set_status_moving(false);
        self.stop_moving();
        self.target = None;
        self.target_location = None;
        self.set_status_force_attack(false);
        self.set_ai_state(AIState::Idle);
    }

    /// Apply USA Pilot recrew residual onto this unmanned vehicle.
    ///
    /// Clears DISABLED_UNMANNED, transfers team to pilot team, merges pilot
    /// veterancy (retail VeterancyCrateCollide IsPilot + AddsOwnerVeterancy).
    /// Caller destroys the pilot infantry.
    pub fn apply_pilot_recrew(
        &mut self,
        pilot_team: Team,
        pilot_level: crate::game_logic::VeterancyLevel,
    ) -> bool {
        use crate::game_logic::host_usa_pilot::{merged_recrew_veterancy, veterancy_rank};

        if !self.status.disabled_unmanned {
            return false;
        }
        self.set_status_disabled_unmanned(false);
        self.status.unmanned_owner_team = None;
        self.set_status_disabled_hacked(false);
        self.status.disabled_hacked_until_frame = 0;
        self.set_status_disabled_emp(false);
        self.status.disabled_emp_until_frame = 0;
        self.set_status_disabled_paralyzed(false);
        self.status.disabled_paralyzed_until_frame = 0;
        self.status.attacking = false;
        self.set_status_moving(false);
        self.stop_moving();
        self.target = None;
        self.target_location = None;
        self.set_status_force_attack(false);
        self.set_ai_state(AIState::Idle);
        self.set_team(pilot_team);

        let previous = self.experience.level;
        let merged = merged_recrew_veterancy(previous, pilot_level);
        let transferred = veterancy_rank(merged) > veterancy_rank(previous);
        if merged != previous {
            self.experience.level = merged;
            self.apply_veterancy_bonuses(previous, merged);
        }
        transferred
    }

    /// Apply DISABLED_HACKED residual until `until_frame` (absolute host logic frame).
    /// C++ SpecialAbilityUpdate: setDisabledUntil(DISABLED_HACKED, now + EffectDuration).

    /// C++ Drawable::flashAsSelected residual (default white/house flash, decay 4).
    /// C++ OBJECT_STATUS_DEPLOYED residual.
    pub fn is_deployed(&self) -> bool {
        self.status.deployed
    }

    /// Toggle DeployStyle residual (artillery / missile humvee / etc.).
    pub fn set_deployed(&mut self, deployed: bool) {
        // Wave 203: status last-writer via host_status_log::record_deployed.
        self.set_status_deployed(deployed);
        if deployed {
            // Deployed units typically stop locomoting residual.
            self.stop_moving();
            self.set_status_moving(false);
        }
    }

    /// Install C++ DeployStyleAIUpdate from the exact Object INI module data
    /// retained on this object's template.  A vehicle-looking template with
    /// no authored DeployStyle behavior must remain an ordinary mobile unit.
    pub fn install_deploy_style_if_needed(&mut self) {
        if self.deploy_style.is_some() {
            return;
        }
        if let Some(metadata) = self.get_template().deploy_style_metadata.as_ref() {
            self.deploy_style = Some(
                crate::game_logic::host_deploy_style::HostDeployStyleData::from_metadata(metadata),
            );
        }
    }

    /// C++ TensileFormationUpdate install residual (AvalancheChunk peels).
    pub fn install_tensile_formation_if_needed(&mut self) {
        if self.tensile_formation.is_some() {
            return;
        }
        if let Some(data) =
            crate::game_logic::host_tensile_formation::HostTensileFormationData::for_template(
                &self.template_name,
            )
        {
            self.tensile_formation = Some(data);
        }
    }

    pub fn install_fire_spread_if_needed(&mut self) {
        if self.fire_spread.is_some() {
            return;
        }
        if let Some(data) = crate::game_logic::host_fire_spread::HostFireSpreadData::for_template(
            &self.template_name,
        ) {
            self.fire_spread = Some(data);
        }
    }

    /// C++ Object::setShroudRange residual (ActiveShroudUpgrade).
    pub fn set_shroud_range(&mut self, new_range: f32) {
        self.shroud_range =
            crate::game_logic::host_active_shroud_upgrade::apply_active_shroud_range(
                self.shroud_range,
                new_range,
            );
    }

    pub fn install_animation_steering_if_needed(&mut self) {
        if self.animation_steering.is_some() {
            return;
        }
        if let Some(data) =
            crate::game_logic::host_animation_steering::HostAnimationSteeringData::for_template(
                &self.template_name,
            )
        {
            self.animation_steering = Some(data);
        }
    }

    pub fn install_float_update_if_needed(&mut self) {
        if self.float_update.is_some() {
            return;
        }
        if let Some(data) = crate::game_logic::host_float_update::HostFloatUpdateData::for_template(
            &self.template_name,
        ) {
            self.float_update = Some(data);
        }
    }

    pub fn install_prone_update_if_needed(&mut self) {
        if self.prone_update.is_some() {
            return;
        }
        if let Some(data) = crate::game_logic::host_prone_update::HostProneUpdateData::for_template(
            &self.template_name,
        ) {
            self.prone_update = Some(data);
        }
    }

    pub fn install_radius_decal_update_if_needed(&mut self) {
        if self.radius_decal_update.is_some() {
            return;
        }
        if let Some(data) =
            crate::game_logic::host_radius_decal_update::HostRadiusDecalUpdateData::for_template(
                &self.template_name,
            )
        {
            self.radius_decal_update = Some(data);
        }
    }

    pub fn install_checkpoint_update_if_needed(&mut self) {
        if self.checkpoint_update.is_some() {
            return;
        }
        if let Some(mut data) =
            crate::game_logic::host_checkpoint_update::HostCheckpointUpdateData::for_template(
                &self.template_name,
                self.vision_range,
            )
        {
            data.vision_range = self.vision_range.max(data.vision_range);
            self.checkpoint_update = Some(data);
        }
    }

    pub fn install_spectre_gunship_deployment_if_needed(&mut self) {
        if self.spectre_gunship_deployment.is_some() {
            return;
        }
        if let Some(data) =
            crate::game_logic::host_spectre_gunship_deployment::HostSpectreGunshipDeploymentData::for_template(
                &self.template_name,
            )
        {
            self.spectre_gunship_deployment = Some(data);
        }
    }

    pub fn install_smart_bomb_target_homing_if_needed(&mut self) {
        if self.smart_bomb_target_homing.is_some() {
            return;
        }
        if let Some(data) =
            crate::game_logic::host_smart_bomb_target_homing::HostSmartBombTargetHomingData::for_template(
                &self.template_name,
            )
        {
            self.smart_bomb_target_homing = Some(data);
        }
    }

    pub fn set_smart_bomb_target(&mut self, target: glam::Vec3) -> bool {
        self.install_smart_bomb_target_homing_if_needed();
        self.smart_bomb_target_homing
            .as_mut()
            .map(|h| h.set_target_position(target))
            .unwrap_or(false)
    }

    pub fn create_delivery_radius_decal(&mut self, pos: glam::Vec3, frame: u32) -> bool {
        self.install_radius_decal_update_if_needed();
        let Some(rd) = self.radius_decal_update.as_mut() else {
            return false;
        };
        let tmpl =
            crate::game_logic::host_radius_decal_update::default_delivery_decal_template_for_host(
                &self.template_name,
            );
        let radius =
            crate::game_logic::host_radius_decal_update::default_delivery_decal_radius_for_template(
                &self.template_name,
            );
        rd.create_radius_decal(tmpl, radius, pos, frame);
        rd.set_kill_when_no_longer_attacking(true);
        !rd.delivery_decal.is_empty()
    }

    pub fn install_enemy_near_if_needed(&mut self) {
        if self.enemy_near.is_some() {
            return;
        }
        if let Some(data) = crate::game_logic::host_enemy_near::HostEnemyNearData::for_template(
            &self.template_name,
            self.vision_range,
        ) {
            self.enemy_near = Some(data);
        }
    }

    pub fn install_base_regenerate_if_needed(&mut self) {
        if self.base_regenerate.is_some() {
            return;
        }
        let is_structure = self.is_kind_of(crate::game_logic::KindOf::Structure);
        if let Some(data) =
            crate::game_logic::host_base_regenerate::HostBaseRegenerateData::for_structure_template(
                &self.template_name,
                is_structure,
            )
        {
            self.base_regenerate = Some(data);
        }
    }

    pub fn notify_base_regenerate_damage(&mut self, current_frame: u32, is_healing: bool) {
        if let Some(br) = self.base_regenerate.as_mut() {
            br.on_damage(current_frame, is_healing);
        }
    }

    pub fn has_fire_spread(&self) -> bool {
        self.fire_spread.is_some()
    }

    pub fn try_ignite_fire_spread(&mut self, current_frame: u32) -> bool {
        let Some(fs) = self.fire_spread.as_mut() else {
            return false;
        };
        fs.try_to_ignite(current_frame)
    }

    pub fn has_tensile_formation(&self) -> bool {
        self.tensile_formation.is_some()
    }

    /// Health fraction for BODY_DAMAGED residual gate.
    pub fn health_fraction(&self) -> f32 {
        let max_h = self.health.maximum.max(self.max_health).max(1.0);
        (self.health.current / max_h).clamp(0.0, 1.0)
    }

    /// True when DeployStyle residual allows firing this frame.
    pub fn deploy_style_allows_fire(&self) -> bool {
        match self.deploy_style.as_ref() {
            None => true,
            Some(d) => d.is_ready_to_attack(),
        }
    }

    /// True when DeployStyle residual allows pathing this frame.
    pub fn deploy_style_allows_move(&self) -> bool {
        match self.deploy_style.as_ref() {
            None => true,
            Some(d) => d.is_ready_to_move(),
        }
    }

    /// C++ CommandButtonHuntUpdate::setCommandButton residual.
    pub fn start_command_button_hunt(
        &mut self,
        mode: crate::game_logic::host_command_button_hunt::HostCommandButtonHuntMode,
        current_frame: u32,
    ) {
        self.command_button_hunt = Some(
            crate::game_logic::host_command_button_hunt::HostCommandButtonHuntData::new(
                mode,
                current_frame,
            ),
        );
        self.set_ai_state(AIState::Idle);
        self.target = None;
        self.stop_moving();
    }

    pub fn clear_command_button_hunt(&mut self) {
        if let Some(h) = self.command_button_hunt.as_mut() {
            h.clear();
        }
        self.command_button_hunt = None;
    }

    pub fn flash_as_selected(&mut self) {
        self.selection_flash_remaining =
            crate::game_logic::host_saboteur::SABOTEUR_FLASH_DECAY_FRAMES;
        self.record_host_ai_request();
    }

    /// True while selection flash envelope residual is active.
    pub fn is_selection_flashing(&self) -> bool {
        self.selection_flash_remaining > 0
    }

    /// Tick selection flash residual once per logic frame.
    pub fn tick_selection_flash(&mut self) {
        self.selection_flash_remaining = self.selection_flash_remaining.saturating_sub(1);
        self.record_host_ai_request();
    }

    pub fn apply_disabled_hacked(&mut self, until_frame: u32) {
        self.set_status_disabled_hacked(true);
        self.status.disabled_hacked_until_frame = until_frame;
        self.record_disable_timers();
        self.status.attacking = false;
        self.set_status_moving(false);
        self.stop_moving();
        self.target = None;
        self.target_location = None;
        self.set_status_force_attack(false);
        self.set_ai_state(AIState::Idle);
    }

    /// Expire DISABLED_HACKED when the host frame passes the residual timer.
    pub fn tick_disabled_hacked(&mut self, current_frame: u32) {
        if self.status.disabled_hacked
            && self.status.disabled_hacked_until_frame > 0
            && current_frame >= self.status.disabled_hacked_until_frame
        {
            self.set_status_disabled_hacked(false);
            self.status.disabled_hacked_until_frame = 0;
        }
    }

    /// Apply DISABLED_EMP residual until `until_frame` (absolute host logic frame).
    /// C++ EMPUpdate::doDisableAttack: setDisabledUntil(DISABLED_EMP, now + DisabledDuration).
    /// Refresh extends the timer if a later expiry is provided.

    /// Disable until_frame residual → GameWorld SetDisableTimers.
    pub fn record_disable_timers(&mut self) {
        crate::game_logic::host_disable_timers_log::record(
            self.id,
            self.status.disabled_emp_until_frame,
            self.status.disabled_hacked_until_frame,
            self.status.disabled_paralyzed_until_frame,
        );
    }

    pub fn apply_disabled_emp(&mut self, until_frame: u32) {
        self.set_status_disabled_emp(true);
        if until_frame > self.status.disabled_emp_until_frame {
            self.status.disabled_emp_until_frame = until_frame;
        }
        self.record_disable_timers();
        self.set_status_attacking(false);
        self.set_status_moving(false);
        self.stop_moving();
        self.target = None;
        self.target_location = None;
        self.set_status_force_attack(false);
        self.set_ai_state(AIState::Idle);
    }

    /// Expire DISABLED_EMP when the host frame passes the residual timer.
    pub fn tick_disabled_emp(&mut self, current_frame: u32) {
        if self.status.disabled_emp
            && self.status.disabled_emp_until_frame > 0
            && current_frame >= self.status.disabled_emp_until_frame
        {
            self.set_status_disabled_emp(false);
            self.status.disabled_emp_until_frame = 0;
        }
    }

    /// Apply DISABLED_PARALYZED residual until `until_frame` (absolute host logic frame).
    /// C++ BattlePlanUpdate::paralyzeTroop: setDisabledUntil(DISABLED_PARALYZED, now + frames).
    /// Refresh extends the timer if a later expiry is provided.
    pub fn apply_disabled_paralyzed(&mut self, until_frame: u32) {
        self.set_status_disabled_paralyzed(true);
        if until_frame > self.status.disabled_paralyzed_until_frame {
            self.status.disabled_paralyzed_until_frame = until_frame;
        }
        self.record_disable_timers();
        self.status.attacking = false;
        self.set_status_moving(false);
        self.stop_moving();
        self.target = None;
        self.target_location = None;
        self.set_status_force_attack(false);
        self.set_ai_state(AIState::Idle);
    }

    /// Expire DISABLED_PARALYZED when the host frame passes the residual timer.
    pub fn tick_disabled_paralyzed(&mut self, current_frame: u32) {
        if self.status.disabled_paralyzed
            && self.status.disabled_paralyzed_until_frame > 0
            && current_frame >= self.status.disabled_paralyzed_until_frame
        {
            self.set_status_disabled_paralyzed(false);
            self.status.disabled_paralyzed_until_frame = 0;
        }
    }

    /// C++ goInvulnerable residual (OCL InvulnerableTime post-eject).
    pub fn is_eject_invulnerable(&self) -> bool {
        self.status.eject_invulnerable
    }

    /// Apply InvulnerableTime residual until `until_frame` (absolute host logic frame).
    /// Refresh extends the timer if a later expiry is provided.
    pub fn apply_eject_invulnerable(&mut self, until_frame: u32) {
        self.set_status_eject_invulnerable(true);
        if until_frame > self.status.eject_invulnerable_until_frame {
            self.status.eject_invulnerable_until_frame = until_frame;
        }
        // C++ goInvulnerable uses defection helper without defector FX flash.
        let now = crate::game_logic::host_historic_bonus::logic_frame();
        let frames = until_frame.saturating_sub(now).max(1);
        self.begin_undetected_defection(
            now,
            frames.min(crate::game_logic::host_defection_helper::DEFECTION_DETECTION_TIME_MAX),
            false,
        );
    }

    /// Expire InvulnerableTime when the host frame passes the residual timer.
    /// Host residual: OCL_EjectPilotViaParachute parachuting state.
    pub fn is_parachuting(&self) -> bool {
        self.status.parachuting
    }

    /// Whether AmericaParachute residual chute is open (past OpenDist freefall).
    pub fn is_parachute_open(&self) -> bool {
        self.status.parachute_open
    }

    /// Begin air-eject parachute residual (elevated spawn + freefall → OpenDist → open).
    ///
    /// Applies C++ low-altitude open fudge: if height above ground < 2×OpenDist,
    /// fudge start height so the chute can still open.
    pub fn apply_eject_parachuting(&mut self) {
        use crate::game_logic::host_usa_pilot::fudge_parachute_start_height;
        let start_y = self.get_position().y;
        let ground_y = 0.0; // host residual ground plane
        let fudged = fudge_parachute_start_height(start_y, ground_y);
        self.set_status_parachuting(true);
        self.status.airborne_target = true;
        self.set_status_parachute_open(false);
        self.status.parachute_start_height = fudged;
        // Freefall residual: pitch/roll rates seed only when chute opens.
        self.status.parachute_pitch = 0.0;
        self.status.parachute_roll = 0.0;
        self.status.parachute_pitch_rate = 0.0;
        self.status.parachute_roll_rate = 0.0;
    }

    /// Begin AmericaCrateParachute residual for cargo crate payload.
    ///
    /// Uses crate OpenDist **12.5** low-altitude fudge (not pilot OpenDist 100).
    /// Fail-closed: not full PutInContainer AmericaCrateParachute Object.
    pub fn apply_crate_parachuting(&mut self) {
        use crate::game_logic::host_deliver_payload::fudge_crate_parachute_start_height;
        let start_y = self.get_position().y;
        let ground_y = 0.0;
        let fudged = fudge_crate_parachute_start_height(start_y, ground_y);
        self.set_status_parachuting(true);
        self.status.airborne_target = true;
        self.set_status_parachute_open(false);
        self.status.parachute_start_height = fudged;
        self.status.parachute_pitch = 0.0;
        self.status.parachute_roll = 0.0;
        self.status.parachute_pitch_rate = 0.0;
        self.status.parachute_roll_rate = 0.0;
    }

    /// Whether low-altitude open fudge residual applied for this parachute start.
    pub fn parachute_start_was_fudged(&self) -> bool {
        use crate::game_logic::host_usa_pilot::parachute_start_height_was_fudged;
        // Fudge rewrites start height; detect by comparing raw y vs stored start.
        // After apply, start_height is fudged value; raw spawn y is current y
        // only at apply time — host honesty uses registry counter instead.
        parachute_start_height_was_fudged(self.get_position().y, 0.0)
    }

    /// Mark AmericaParachute residual chute open (after OpenDist freefall).
    ///
    /// Seeds pitch/roll rates residual (C++ constructor random in ±Pitch/RollRateMax;
    /// host uses deterministic mid residual).
    pub fn open_eject_parachute(&mut self) {
        use crate::game_logic::host_usa_pilot::{
            parachute_initial_pitch_rate, parachute_initial_roll_rate,
        };
        self.set_status_parachute_open(true);
        self.status.parachute_pitch = 0.0;
        self.status.parachute_roll = 0.0;
        self.status.parachute_pitch_rate = parachute_initial_pitch_rate();
        self.status.parachute_roll_rate = parachute_initial_roll_rate();
    }

    /// Clear parachuting residual on land.
    pub fn clear_eject_parachuting(&mut self) {
        self.set_status_parachuting(false);
        self.status.airborne_target = false;
        self.set_status_parachute_open(false);
        self.status.parachute_start_height = 0.0;
        self.status.parachute_pitch = 0.0;
        self.status.parachute_roll = 0.0;
        self.status.parachute_pitch_rate = 0.0;
        self.status.parachute_roll_rate = 0.0;
        self.status.parachute_landing_override = None;
        self.set_status_parachute_landing_override_set(false);
    }

    /// C++ ParachuteContain::setOverrideDestination residual.
    ///
    /// DeliverPayload aims the open chute at an explicit LZ instead of
    /// findPositionAround drift. Host residual: store XZ target for open-chute
    /// horizontal step.
    pub fn set_parachute_override_destination(&mut self, dest: glam::Vec3) {
        self.status.parachute_landing_override = Some(dest);
        self.set_status_parachute_landing_override_set(true);
    }

    /// Whether landing override residual is armed.
    pub fn has_parachute_landing_override(&self) -> bool {
        self.status.parachute_landing_override_set
            && self.status.parachute_landing_override.is_some()
    }

    /// Landing override residual target (world XZ; y ignored for aim).
    pub fn parachute_landing_override(&self) -> Option<glam::Vec3> {
        if self.status.parachute_landing_override_set {
            self.status.parachute_landing_override
        } else {
            None
        }
    }

    /// AmericaParachute pitch residual (radians) while chute open.
    pub fn parachute_pitch(&self) -> f32 {
        self.status.parachute_pitch
    }

    /// AmericaParachute roll residual (radians) while chute open.
    pub fn parachute_roll(&self) -> f32 {
        self.status.parachute_roll
    }

    pub fn tick_eject_invulnerable(&mut self, current_frame: u32) {
        if self.status.eject_invulnerable
            && self.status.eject_invulnerable_until_frame > 0
            && current_frame >= self.status.eject_invulnerable_until_frame
        {
            self.set_status_eject_invulnerable(false);
            self.status.eject_invulnerable_until_frame = 0;
        }
    }
}
