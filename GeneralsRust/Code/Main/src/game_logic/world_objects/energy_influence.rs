//! Immediate receiving Energy entry and synchronous brownout callbacks.
use super::super::*;
use crate::game_logic::object::{ObjectPanelFlag, ScriptDisable, TimedDisable};

#[derive(Clone, Copy)]
struct DisableUntil {
    effect: TimedDisable,
    deadline: u32,
}

impl GameLogic {
    /// EMPUpdate calls Object::setDisabledUntil on the installed target.
    pub(in crate::game_logic) fn apply_owned_emp_disabled(
        &mut self,
        id: ObjectId,
        until: u32,
    ) -> bool {
        self.apply_owned_initial_disabled(
            id,
            DisableUntil {
                effect: TimedDisable::Emp,
                deadline: until,
            },
        )
    }

    /// SpecialAbilityUpdate calls Object::setDisabledUntil on its target.
    pub(in crate::game_logic) fn apply_owned_hacked_disabled(
        &mut self,
        id: ObjectId,
        until: u32,
    ) -> bool {
        self.apply_owned_initial_disabled(
            id,
            DisableUntil {
                effect: TimedDisable::Hacked,
                deadline: until,
            },
        )
    }

    /// BattlePlanUpdate calls Object::setDisabledUntil on the affected unit.
    pub(in crate::game_logic) fn apply_owned_paralyzed_disabled(
        &mut self,
        id: ObjectId,
        until: u32,
    ) -> bool {
        self.apply_owned_initial_disabled(
            id,
            DisableUntil {
                effect: TimedDisable::Paralyzed,
                deadline: until,
            },
        )
    }

    /// ScriptActions.cpp:3875–3880 uses setDisabledUntil for both values.
    pub(in crate::game_logic) fn set_owned_named_held(&mut self, id: ObjectId, held: bool) {
        use game_engine::common::game_common::{FOREVER, NEVER};
        let deadline = if held { FOREVER } else { NEVER };
        self.apply_owned_initial_disabled(id, DisableUntil { effect: TimedDisable::Held, deadline });
    }

    /// Contain callbacks release a passenger with clearDisabled, which
    /// clears inherited masks before firing the recovering passenger's edge.
    pub(in crate::game_logic) fn set_owned_passenger_held(
        &mut self,
        id: ObjectId,
        held: bool,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        if held {
            self.apply_owned_initial_disabled_with_effects(
                id,
                DisableUntil {
                    effect: TimedDisable::Held,
                    deadline: game_engine::common::game_common::FOREVER,
                },
                effects,
            );
        } else {
            self.clear_owned_disabled(id, TimedDisable::Held);
        }
    }

    pub(in crate::game_logic) fn set_owned_script_disable(
        &mut self,
        id: ObjectId,
        flag: ScriptDisable,
        disabled: bool,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let Some(object) = self.objects.get_mut(&id) else {
            return;
        };
        let script_bit = flag.flag_mut(&mut object.status);
        if *script_bit == disabled {
            return;
        }
        *script_bit = disabled;
        // Object::setScriptStatus dirties the owner's partition before the
        // actual mask callback. Inherited rider/spawn masks skip this step.
        self.partition_manager.mark_receiving_object_dirty(
            id.0,
            &object.visual_draw_identity_token,
            true,
        );
        let effect = flag.effect();
        if disabled {
            self.apply_owned_initial_disabled_with_effects(
                id,
                DisableUntil {
                    effect,
                    deadline: game_engine::common::game_common::FOREVER,
                },
                effects,
            );
        } else {
            self.clear_owned_disabled(id, effect);
        }
    }

    pub(in crate::game_logic) fn apply_owned_object_panel_flag(
        &mut self,
        id: ObjectId,
        flag: &str,
        value: bool,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let Some(flag) = ObjectPanelFlag::parse(flag) else { return };
        match flag {
            ObjectPanelFlag::Enabled => {
                self.set_owned_script_disable(id, ScriptDisable::Disabled, !value, effects);
            }
            ObjectPanelFlag::Powered => {
                self.set_owned_script_disable(id, ScriptDisable::Underpowered, !value, effects);
            }
            ObjectPanelFlag::Indestructible => self.set_object_indestructible(id, value),
            ObjectPanelFlag::Unsellable => {
                if let Some(object) = self.objects.get_mut(&id) {
                    object.set_script_unsellable(value);
                }
            }
            ObjectPanelFlag::PlayerTargetable => {
                if let Some(object) = self.objects.get_mut(&id) {
                    object.set_script_targetable(value);
                }
            }
        }
    }

    fn apply_owned_initial_disabled(
        &mut self,
        id: ObjectId,
        request: DisableUntil,
    ) -> bool {
        self.apply_owned_initial_disabled_with_effects(id, request, &mut Default::default())
    }

    fn apply_owned_initial_disabled_with_effects(
        &mut self,
        id: ObjectId,
        request: DisableUntil,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let epoch = self.visual_world_epoch;
        let (birth, disable_callback, deadline_changed, fold_adjustment) = {
            let Some(object) = self.objects.get_mut(&id) else {
                return false;
            };
            let was_disabled = object.has_disabled_type();
            let before = Self::folded_enabled_production(object);
            if request.effect.is_power_style() && !object.is_power_style_disabled() {
                object.queue_power_disable_misc_audio(true, &mut self.health_events);
            }
            let changed = object.begin_owned_timed_disable(request.effect, request.deadline, self.frame);
            let after = Self::folded_enabled_production(object);
            (
                object.visual_object_generation,
                !was_disabled,
                changed,
                object.owner_player_id.map(|owner| (owner, before.wrapping_sub(after))),
            )
        };
        if let Some((owner, adjustment)) = fold_adjustment
            && adjustment != 0
            && let Some(player) = self.players.get_mut(&owner)
        {
            // A status change alone has no Energy effect. Cancel its effect on
            // the legacy fold before child callbacks can observe the new mask.
            player.power_production_adjustment =
                player.power_production_adjustment.wrapping_add(adjustment);
        }
        if deadline_changed {
            // C++ ContainModuleInterface::friend_getRider precedes SpawnBehavior.
            if let Some(rider) = self.containment_disable_rider(id) {
                self.apply_owned_initial_disabled_with_effects(rider, request, effects);
                if !self.spawn_identity_matches(id, birth, epoch) {
                    return true;
                }
            }
            if let Some(children) = self
                .host_object(id)
                .filter(|object| object.is_spawns_are_the_weapons())
                .map(Object::spawn_behavior_children)
                .filter(|children| !children.is_empty())
            {
                self.apply_owned_spawn_children_disabled_until(
                    id, birth, epoch, children, request, effects,
                );
                if !self.spawn_identity_matches(id, birth, epoch) {
                    return true;
                }
            }
        }

        // Object::setDisabledUntil calls onDisabledEdge after the children.
        // An earlier queued edge could be drained by a child's brownout.
        if disable_callback {
            // Object::onDisabledEdge cancels an entered Dozer task before Radar and Energy.
            use gamelogic::object::update::ai_update::dozer_ai_update::DozerTask;
            let dozer_task = self.host_object(id).and_then(|object| {
                let task = object.dozer_tasks.current;
                (task != DozerTask::Invalid
                    && object.get_template().receiving_client_template.as_ref()
                        .and_then(|template| template.dozer_ai_is_worker())
                        .is_some())
                .then_some(task)
            });
            if let Some(task) = dozer_task {
                let cancel = self.dozer_cancel_task_owned(id, task);
                if !self.spawn_identity_matches(id, birth, epoch) {
                    return true;
                }
                if let Err(error) = cancel {
                    // The task request is already cleared. Keep the primary
                    // reset and finish the Object edge if the inherited Idle failed.
                    self.dozer_primary_reset_owned(id);
                    log::error!("Dozer {id:?} could not enter idle while disabling: {error}");
                }
            }
            let object = self.objects.get_mut(&id).unwrap();
            object.on_disabled_edge(true);
        }

        self.admit_host_radar_disabled_edges(id);
        if disable_callback {
            self.admit_producer_energy_without_mask_change(id, true);
        }
        true
    }

    fn folded_enabled_production(object: &Object) -> i32 {
        if Self::object_contributes_power(object) && !object.has_disabled_type() {
            return object.producer_energy_on_disabled_edge().unwrap_or(0);
        }
        0
    }

    /// Apply an original callback whose mask change has already been accounted
    /// for, including a named HELD command or an outer clearDisabled callback.
    pub(in crate::game_logic) fn admit_producer_energy_without_mask_change(
        &mut self,
        id: ObjectId,
        disabled: bool,
    ) {
        let Some((owner, energy)) = self.objects.get(&id).and_then(|object| {
            object
                .owner_player_id
                .zip(object.producer_energy_on_disabled_edge())
        }) else {
            return;
        };
        self.signal_producer_energy_edge(owner, energy, 0, !disabled);
        self.record_power_projection(owner);
    }

    pub(in crate::game_logic) fn containment_disable_rider(&self, owner: ObjectId) -> Option<ObjectId> {
        let object = self.host_object(owner)?;
        if object.get_template().contain_module.kind == crate::game_logic::ContainModuleKind::RiderChange {
            let &rider = object.contained_unit_ids().first()?;
            return self.host_object(rider)
                .filter(|rider| rider.contained_by == Some(owner))
                .map(|rider| rider.id);
        }
        if object.admits_ocl_portable_structure() {
            return self.overlord_helix_portable_occupant_id(owner);
        }
        None
    }

    /// C++ `SpawnBehavior::orderSlavesDisabledUntil`: visit the receiving
    /// parent's ordered child IDs, idle each child's AI first, then apply the
    /// same disable deadline to that child before advancing to the next one.
    fn apply_owned_spawn_children_disabled_until(
        &mut self,
        parent_id: ObjectId,
        parent_birth: u64,
        epoch: u64,
        children: Vec<(ObjectId, u64)>,
        request: DisableUntil,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        for (child_id, child_birth) in children {
            if !self.spawn_identity_matches(parent_id, parent_birth, epoch) {
                return;
            }
            let still_exact_child = |world: &Self| {
                world.spawn_identity_matches(child_id, child_birth, epoch)
                    && world.host_object(parent_id).is_some_and(|parent| {
                        parent.contains_spawn_behavior_child(child_id, &child_birth)
                    })
            };
            if !still_exact_child(self) {
                continue;
            }

            // aiIdle can reenter Main before setDisabledUntil.
            if let Err(error) = self.unit_command_ai_idle_owned_with_effects(child_id, effects) {
                log::error!("Spawned Object {child_id:?} failed its Idle callback: {error}");
            }
            if !self.spawn_identity_matches(parent_id, parent_birth, epoch) {
                return;
            }
            if !still_exact_child(self) {
                continue;
            }
            self.apply_owned_initial_disabled_with_effects(child_id, request, effects);
        }
    }

    /// Original constructor setTeam -> becomingTeamMember, before onCreate.
    /// Only the explicit receiving Player owns this mutation; faction is not a
    /// fallback and loading has a separate inert roster-admission operation.
    pub(in crate::game_logic) fn admit_object_energy_influence(&mut self, id: ObjectId) {
        self.adjust_object_team_energy(id, true);
    }

    /// Object::setOrRestoreTeam calls the old controller after removing its
    /// membership, then the new controller after admitting its membership.
    pub(super) fn adjust_object_team_energy(&mut self, id: ObjectId, incoming: bool) {
        let Some((owner, energy)) = self.objects.get(&id).and_then(|object| {
            object.owner_player_id.zip(Self::team_membership_energy(object))
        }) else {
            return;
        };
        self.signal_template_energy_delta(owner, energy, incoming);
    }

    pub(super) fn admit_retiring_object_energy(&mut self, object: &Object, owner: u32) {
        if let Some(energy) = Self::team_membership_energy(object) {
            self.signal_template_energy_delta(owner, energy, false);
            self.record_power_projection(owner);
        }
    }

    /// Player.cpp:1028–1037 excludes construction from Team membership
    /// accounting. Player's completion callback admits that energy separately.
    fn team_membership_energy(object: &Object) -> Option<i32> {
        if object.status.under_construction {
            return None;
        }
        Self::template_energy_for_player(object)
    }

    /// Object.cpp:3770–3784 excludes disabled producers from influence.
    /// Consumers continue to draw power regardless of the disabled mask.
    pub(super) fn template_energy_for_player(object: &Object) -> Option<i32> {
        let energy = object.get_template().energy_production?;
        (energy < 0 || (energy > 0 && !object.has_disabled_type())).then_some(energy)
    }

    pub(super) fn signal_template_energy_delta(&mut self, owner: u32, energy: i32, incoming: bool) {
        let Some(player) = self.players.get_mut(&owner) else {
            return;
        };
        let brownout = if incoming {
            player.add_template_energy(energy, self.frame)
        } else {
            player.remove_template_energy(energy, self.frame)
        };
        if let Some(brownout) = brownout {
            self.admit_player_energy_brownout(owner, brownout);
        }
    }

    /// Apply the Energy part of Object::onDisabledEdge after the caller has
    /// finished the object's module and radar callbacks.
    pub(in crate::game_logic) fn admit_owned_producer_energy_edge(
        &mut self,
        id: ObjectId,
        disabled: bool,
    ) {
        let Some((owner, power, folded_output)) = self.objects.get(&id).and_then(|object| {
            let owner = object.owner_player_id?;
            let power = object.producer_energy_on_disabled_edge()?;
            Some((owner, power, Self::folded_producer_output(object, power)))
        }) else {
            return;
        };
        self.signal_producer_energy_edge(owner, power, folded_output, !disabled);
        self.record_power_projection(owner);
    }

    fn folded_producer_output(object: &Object, power: i32) -> i32 {
        if Self::object_contributes_power(object) {
            power
        } else {
            0
        }
    }

    /// Energy.cpp:79–98 puts a negative adjustPower argument in consumption.
    /// Account for the difference from the remaining compatibility fold before
    /// the synchronous brownout callback can observe the producer's new mask.
    fn signal_producer_energy_edge(
        &mut self,
        owner: u32,
        power: i32,
        folded_output: i32,
        incoming: bool,
    ) {
        let production_difference = power.max(0).wrapping_sub(folded_output);
        let consumption_difference = power.min(0).wrapping_neg();
        if (production_difference != 0 || consumption_difference != 0)
            && let Some(player) = self.players.get_mut(&owner)
        {
            let direction = if incoming { 1 } else { -1 };
            player.power_production_adjustment = player
                .power_production_adjustment
                .wrapping_add(production_difference.wrapping_mul(direction));
            player.power_consumption_adjustment = player
                .power_consumption_adjustment
                .wrapping_add(consumption_difference.wrapping_mul(direction));
        }
        self.signal_template_energy_delta(owner, power, incoming);
    }

    /// Admit a signed EnergyBonus to its exact Player, then settle radar and
    /// powered objects before returning to the caller's module callback.
    pub(super) fn signal_power_bonus(&mut self, owner: Option<u32>, delta: i32) {
        let Some(owner) = owner else { return };
        let Some(player) = self.players.get_mut(&owner) else {
            return;
        };
        let brownout = player.adjust_power_bonus(delta, self.frame);
        self.admit_player_energy_brownout(owner, brownout);
        self.record_power_projection(owner);
    }

    /// Energy.cpp:157–216 signals bonuses even if a plant is disabled.
    /// Run the synchronous callback before updating the module's gross output.
    pub(in crate::game_logic) fn change_object_power_bonus(
        &mut self,
        id: ObjectId,
        delta: i32,
    ) -> Option<&mut Object> {
        let owner = self.objects.get(&id)?.owner_player_id;
        self.signal_power_bonus(owner, delta);
        let object = self.objects.get_mut(&id)?;
        object.power_provided = object.power_provided.wrapping_add(delta);
        // A brownout callback can reenable or disable this Object. Use
        // its settled state so the following fold counts the delta once.
        if object.has_disabled_type()
            && Self::omit_disabled_production_from_grid(object)
            && let Some(player) = owner.and_then(|id| self.players.get_mut(&id))
        {
            player.power_production_adjustment =
                player.power_production_adjustment.wrapping_add(delta);
        }
        Some(object)
    }

    /// SabotagePowerPlantCrateCollide.cpp:112–120 applies the outage on contact.
    pub(in crate::game_logic) fn sabotage_player_power(&mut self, owner: u32, deadline: u32) {
        let Some(player) = self.players.get_mut(&owner) else {
            return;
        };
        player.power_sabotaged_till_frame = deadline;
        player.refresh_energy_projection_at(self.frame);
        self.admit_player_energy_brownout(owner, true);
    }

    /// Player.cpp:698–701 runs after this player's generic Team scripts.
    /// Energy getters recover at equality, while this callback waits one more frame.
    pub(crate) fn expire_player_power_sabotage(&mut self, owner: u32) {
        let Some(player) = self.players.get_mut(&owner) else {
            return;
        };
        let deadline = player.power_sabotaged_till_frame;
        if deadline == 0 || self.frame <= deadline {
            return;
        }
        player.power_sabotaged_till_frame = 0;
        player.refresh_energy_projection_at(self.frame);
        let brownout = !player.has_sufficient_energy_at(self.frame);
        self.admit_player_energy_brownout(owner, brownout);
    }

    /// Player::onPowerBrownOutChange: synchronous exact-owner restriction and
    /// original POWERED member callback, including repeated nonzero signals.
    pub(in crate::game_logic) fn admit_player_energy_brownout(
        &mut self,
        owner: u32,
        brownout: bool,
    ) {
        let Some(player) = self.players.get_mut(&owner) else {
            return;
        };
        let had_radar = player.has_radar();
        if brownout {
            player.disable_radar();
        } else {
            player.enable_radar();
        }
        let has_radar = player.has_radar();
        let count = player.radar_count.max(0) as u32;
        self.record_host_radar_transition(owner, count, had_radar, has_radar);
        // Canonical Player prototype admission -> Team instance -> member
        // prepend order. Release each existing owner loan before callbacks;
        // recursive Energy signals observe the current canonical membership.
        let teams = self
            .team_factory
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .owned_team_instances_for_controller(owner);
        for team in teams {
            let (team_id, ids) = {
                let team = team.read().unwrap_or_else(|error| error.into_inner());
                (team.get_id(), team.get_members().to_vec())
            };
            for id in ids {
                let id = ObjectId(id);
                let powered_member = self.objects.get(&id).is_some_and(|object| {
                    object.owner_player_id == Some(owner)
                        && object.team_instance_id == Some(team_id)
                        && object.is_kind_of(KindOf::Powered)
                });
                if !powered_member {
                    continue;
                }
                if brownout {
                    self.apply_owned_initial_disabled(
                        id,
                        DisableUntil {
                            effect: TimedDisable::Underpowered,
                            deadline: game_engine::common::game_common::FOREVER,
                        },
                    );
                } else {
                    self.clear_owned_disabled(id, TimedDisable::Underpowered);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_panel_flags_deliver_spawn_power_edges_and_keep_script_bits_separate() {
        use game_engine::common::dict::Dict;
        use game_engine::common::well_known_keys::{key_object_enabled, key_object_powered};

        let (mut world, [parent, plant, spawned]) = spawned_radar_world();
        let (peer, peer_ids) = spawned_radar_world();
        assert_eq!([parent, plant, spawned], peer_ids);
        let mut disabled = Dict::new();
        disabled.set_bool(key_object_enabled(), false);
        disabled.set_bool(key_object_powered(), false);
        world.apply_update_obj_values_from_map_properties(parent, &disabled);

        assert_eq!(world.players[&1].power_produced, 0, "same action observes lost power");
        assert_eq!(world.players[&1].radar_count, 0);
        for id in [plant, spawned] {
            let object = &world.objects[&id];
            assert!(object.status.disabled_script_disabled);
            assert!(object.status.disabled_script_underpowered);
            assert!(!object.is_script_disabled());
            assert!(!object.is_script_underpowered(), "inherited mask is independent");
        }
        assert_eq!(peer.players[&1].power_produced, 20);
        assert_eq!(peer.players[&1].radar_count, 1);

        let mut unpowered = Dict::new();
        unpowered.set_bool(key_object_powered(), false);
        world.apply_update_obj_values_from_map_properties(plant, &unpowered);
        assert!(world.objects[&plant].is_script_underpowered());
        let mut enabled = Dict::new();
        enabled.set_bool(key_object_enabled(), true);
        enabled.set_bool(key_object_powered(), true);
        world.apply_update_obj_values_from_map_properties(parent, &enabled);
        assert!(world.objects[&plant].is_script_underpowered(), "child's own script flag");
        assert!(!world.objects[&plant].has_disabled_type(), "inherited clear unwinds the mask");
        assert_eq!(world.players[&1].power_produced, 20);
        assert_eq!(world.players[&1].radar_count, 1);

        let builder = crate::save_load::snapshot::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&world).unwrap();
        let bytes = bincode_legacy::serialize(&snapshot).unwrap();
        let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
        let (mut restored, _) = spawned_radar_world();
        builder.restore_from_snapshot(&decoded, &mut restored).unwrap();
        assert!(restored.objects[&plant].is_script_underpowered());
        assert!(!restored.objects[&plant].has_disabled_type());
        assert_eq!(restored.players[&1].power_produced, 20);
    }

    #[test]
    fn emp_and_hack_keep_orders_and_pause_each_authored_power_until_its_last_disable() {
        use crate::command_system::SpecialPowerType;
        use crate::game_logic::{SpecialPowerModuleKind, SpecialPowerModuleMetadata};

        let power = SpecialPowerType::ParticleCannon;
        let source_index = 3;
        let fixture = || {
            let mut world = GameLogic::new();
            world.add_player(Player::new(1, Team::USA, "DisableOwner", true));
            let mut template = ThingTemplate::new("DisableCooldownProbe");
            template.add_kind_of(KindOf::Vehicle).set_health(100.0);
            template.special_power_modules.push(SpecialPowerModuleMetadata {
                source_index,
                module_tag: Some("ProbePower".into()),
                module_kind: SpecialPowerModuleKind::OclSpecialPower,
                special_power_template: "SpecialPowerParticleCannon".into(),
                special_power_template_id: source_index,
                command_power: Some(power.clone()),
                reload_time_frames: 300,
                required_science: None,
                public_timer: true,
                shared_n_sync: false,
                shortcut_power: false,
                update_module_starts_attack: false,
                starts_paused: false,
                scripted_special_power_only: false,
                cash_bounty: None,
            });
            world.templates.insert(template.name.clone(), template);
            let id = world
                .create_object_for_player("DisableCooldownProbe", 1, Vec3::ZERO)
                .unwrap();
            let object = world.objects.get_mut(&id).unwrap();
            object.authored_special_power_timers.insert(source_index, (8.0, 1));
            object.target_location = Some(Vec3::new(40.0, 0.0, 40.0));
            object.set_status_moving(true);
            (world, id)
        };
        let (mut world, id) = fixture();
        let (peer, peer_id) = fixture();
        assert_eq!(id, peer_id);
        let timer = |world: &GameLogic| world.objects[&id].authored_special_power_timers[&source_index];

        world.set_current_frame(10);
        assert!(world.apply_emp_pulse_disable_field_at(1, Vec3::ZERO, None));
        let emp_deadline = world.objects[&id].status.disabled_emp_until_frame;
        assert_eq!(timer(&world), (8.0, 2), "one pause from the EMP field");
        assert!(world.apply_emp_pulse_disable_field_at(1, Vec3::ZERO, None));
        assert!(world.apply_owned_hacked_disabled(id, 30));
        world.set_owned_named_held(id, true);
        assert_eq!(timer(&world), (8.0, 3), "two disable pauses and one script pause");
        let object = &world.objects[&id];
        assert!(object.status.moving);
        assert_eq!(object.target_location, Some(Vec3::new(40.0, 0.0, 40.0)));
        assert_eq!(timer(&peer), (8.0, 1));

        let builder = crate::save_load::snapshot::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&world).unwrap();
        let bytes = bincode_legacy::serialize(&snapshot).unwrap();
        let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
        let (mut restored, _) = fixture();
        builder.restore_from_snapshot(&decoded, &mut restored).unwrap();

        for owner in [&mut world, &mut restored] {
            assert_eq!(timer(owner), (8.0, 3));
            owner.set_current_frame(30);
            owner.expire_owned_disabled_statuses();
            assert_eq!(timer(owner), (8.0, 2));
            owner.set_owned_named_held(id, false);
            owner.set_current_frame(u64::from(emp_deadline));
            owner.expire_owned_disabled_statuses();
            assert_eq!(timer(owner), (8.0, 1));
            assert!(!owner.objects[&id].has_disabled_type());
            let object = owner.objects.get_mut(&id).unwrap();
            object.pause_special_power_countdown(&power, false);
            object.tick_timers(1.0, &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()));
            assert_eq!(timer(owner), (7.0, 0));
        }
        assert_eq!(timer(&peer), (8.0, 1));
    }

    #[test]
    fn object_load_credits_emp_producer_before_restoring_its_disabled_mask() {
        let fixture = || {
            let mut world = GameLogic::new();
            let source = "
Object LoadOrderPlant
 KindOf = STRUCTURE
 EnergyProduction = 20
 EnergyBonus = 5
 Behavior = OverchargeBehavior Overcharge
  HealthPercentToDrainPerSecond = 0%
  NotAllowedWhenHealthBelowPercent = 0%
 End
End
Object LoadOrderConsumer
 KindOf = STRUCTURE POWERED
 EnergyProduction = -10
End";
            world
                .admit_receiving_module_content(
                    crate::assets::ReceivingObjectContent::from_sources([(
                        "energy-load-order.ini".into(),
                        source.into(),
                    )])
                    .unwrap(),
                )
                .unwrap();
            world.add_player(Player::new(1, Team::USA, "LoadOrder", true));
            world
        };

        for (plant_loaded_last, overcharge) in [(true, false), (false, false), (false, true)] {
            let mut world = fixture();
            let names = if plant_loaded_last {
                ["LoadOrderPlant", "LoadOrderConsumer"]
            } else {
                ["LoadOrderConsumer", "LoadOrderPlant"]
            };
            let ids = names.map(|name| world.create_object_for_player(name, 1, Vec3::ZERO).unwrap());
            let (plant, consumer) = if plant_loaded_last {
                (ids[0], ids[1])
            } else {
                (ids[1], ids[0])
            };
            world.set_current_frame(8);
            if overcharge {
                assert!(world.toggle_overcharge_object(plant));
            }
            assert!(world.apply_owned_emp_disabled(plant, 30));
            assert!(world.objects[&consumer].status.disabled_underpowered);
            assert_eq!(world.players[&1].power_produced, 0);

            let builder = crate::save_load::snapshot::SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&world).unwrap();
            let bytes = bincode_legacy::serialize(&snapshot).unwrap();
            let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
            let mut restored = fixture();
            builder.restore_from_snapshot(&decoded, &mut restored).unwrap();
            for _ in 0..2 {
                assert!(restored.objects[&plant].status.disabled_emp);
                assert_eq!(restored.players[&1].power_produced, if overcharge { 25 } else { 20 });
                assert_eq!(restored.players[&1].power_consumed, 10);
                assert_eq!(
                    restored.objects[&consumer].status.disabled_underpowered,
                    !plant_loaded_last && !overcharge,
                    "the last relevant Team, mask, or upgrade post-process step wins"
                );
                restored.update_player_resources(0.0);
            }
            assert!(world.objects[&consumer].status.disabled_underpowered);
        }
    }

    #[test]
    fn rebuilding_grid_keeps_signed_power_in_registration_order() {
        let mut world = GameLogic::new();
        world.add_player(Player::new(1, Team::USA, "Grid", true));
        world.add_player(Player::new(2, Team::USA, "EmptyGrid", false));
        world.players.get_mut(&1).unwrap().power_production_adjustment = 2;
        world.players.get_mut(&2).unwrap().power_produced = 99;

        // The registry prepends each lifetime. Its newest two producers
        // saturate before the oldest actor subtracts its signed contribution.
        for (id, provided, consumed) in [(9, -7, 3), (2, 10, 4), (17, i32::MAX, 0)] {
            let id = ObjectId(id);
            let mut actor = Object::new(ThingTemplate::new("GridFixture"), id, Team::USA);
            actor.owner_player_id = Some(1);
            actor.construction_percent = 1.0;
            actor.power_provided = provided;
            actor.power_consumed = consumed;
            world.objects.admit(id, actor);
        }

        for _ in 0..2 {
            world.update_player_resources(0.0);
            let player = &world.players[&1];
            assert_eq!(player.power_produced, i32::MAX - 5);
            assert_eq!(player.power_consumed, 7);
            assert_eq!(player.power_available, i32::MAX - 12);
            assert_eq!(world.players[&2].power_available, 0);
        }
    }

    fn spawned_radar_world() -> (GameLogic, [ObjectId; 3]) {
        spawned_radar_world_with_parent_kind("")
    }

    fn spawned_radar_world_with_parent_kind(parent_kind: &str) -> (GameLogic, [ObjectId; 3]) {
        let source = format!(r#"
Object DisableNest
 KindOf = STRUCTURE SPAWNS_ARE_THE_WEAPONS {parent_kind}
 Behavior = DefaultProductionExitUpdate Exit
 End
 Behavior = SpawnBehavior Spawn
  SpawnNumber = 1
  SpawnTemplateName = DisableRadarChild
  OneShot = Yes
 End
End
Object DisableRadarChild
 KindOf = STRUCTURE SPAWNS_ARE_THE_WEAPONS
 EnergyProduction = 20
 Behavior = RadarUpgrade Radar
  TriggeredBy = Upgrade_TestSpawnRadar
  DisableProof = Yes
 End
 Behavior = GrantUpgradeCreate Grant
  UpgradeToGrant = Upgrade_TestSpawnRadar
 End
 Behavior = DefaultProductionExitUpdate Exit
 End
 Behavior = SpawnBehavior Spawn
  SpawnNumber = 1
  SpawnTemplateName = DisableGrandchild
  OneShot = Yes
 End
End
Object DisableGrandchild
 KindOf = INFANTRY
End
"#);
        let mut world = GameLogic::new();
        crate::game_logic::host_upgrade_rules::register_test_upgrade(
            &mut world, "Upgrade_TestSpawnRadar", "OBJECT", 0, 1,
        );
        world
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "spawned-radar.ini".into(),
                    source.into(),
                )])
                .unwrap(),
            )
            .unwrap();
        world.add_player(Player::new(1, Team::GLA, "SpawnOwner", true));
        let parent = world.create_object_for_player("DisableNest", 1, Vec3::ZERO).unwrap();
        for frame in 1..=2 {
            world.set_current_frame(frame);
            world.update_spawn_behavior_with_effects(&mut Default::default()).unwrap();
        }
        let [child] = world.host_object(parent).unwrap().spawn_behavior_children()[..] else {
            panic!("SpawnBehavior must create the radar child");
        };
        let [grandchild] = world.host_object(child.0).unwrap().spawn_behavior_children()[..] else {
            panic!("the child SpawnBehavior must create the grandchild");
        };
        // Only the parent is in range of the EMP. Other actors can only
        // receive it through the authored SpawnBehavior roster.
        for id in [child.0, grandchild.0] {
            assert!(world.set_registered_object_position(id, Vec3::new(2_000.0, 0.0, 0.0)));
        }
        assert_eq!(world.players[&1].power_produced, 20);
        assert_eq!(world.players[&1].radar_count, 1);
        (world, [parent, child.0, grandchild.0])
    }

    #[test]
    fn brownout_reaches_spawn_descendants_before_the_parent_returns() {
        let fixture = || spawned_radar_world_with_parent_kind("POWERED");
        let (mut world, ids) = fixture();
        let (mut restored, peer_ids) = fixture();
        assert_eq!(ids, peer_ids);
        world.set_current_frame(4);
        world.sabotage_player_power(1, 20);
        assert!(ids.iter().all(|id| world.objects[id].status.disabled_underpowered));
        assert_eq!(world.players[&1].power_produced, 0, "child producer callback");
        assert_eq!(world.players[&1].radar_count, 0, "child radar callback");
        assert!(ids.iter().all(|id| !restored.objects[id].status.disabled_underpowered));

        let builder = crate::save_load::snapshot::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&world).unwrap();
        let bytes = bincode_legacy::serialize(&snapshot).unwrap();
        let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
        builder.restore_from_snapshot(&decoded, &mut restored).unwrap();
        for (owner, credited_base) in [(&mut world, 0), (&mut restored, 20)] {
            assert!(ids.iter().all(|id| owner.objects[id].status.disabled_underpowered));
            owner.set_current_frame(21);
            owner.expire_player_power_sabotage(1);
            assert!(ids.iter().all(|id| !owner.objects[id].status.disabled_underpowered));
            assert_eq!(owner.players[&1].power_produced, credited_base + 20);
            assert_eq!(owner.players[&1].radar_count, 1);
            owner.update_player_resources(0.0);
            assert_eq!(owner.players[&1].power_produced, credited_base + 20);
        }
    }

    #[test]
    fn brownout_saves_the_inactive_deadline_at_forever_equality() {
        use game_engine::common::game_common::FOREVER;
        let fixture = || spawned_radar_world_with_parent_kind("POWERED");
        let (mut world, ids) = fixture();
        world.set_current_frame(u64::from(FOREVER));
        world.sabotage_player_power(1, FOREVER + 10);
        let assert_deadlines = |owner: &GameLogic| {
            for id in ids {
                let object = &owner.objects[&id];
                assert!(!object.status.disabled_underpowered);
                assert_eq!(object.underpowered_disable_deadline(), FOREVER);
            }
        };
        assert_deadlines(&world);
        let builder = crate::save_load::snapshot::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&world).unwrap();
        let bytes = bincode_legacy::serialize(&snapshot).unwrap();
        let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
        let (mut restored, _) = fixture();
        builder.restore_from_snapshot(&decoded, &mut restored).unwrap();
        assert_deadlines(&restored);
    }

    #[test]
    fn emp_on_spawn_parent_completes_descendant_radar_and_power_before_returning() {
        let (mut world, ids) = spawned_radar_world();
        let (peer, peer_ids) = spawned_radar_world();
        assert_eq!(ids, peer_ids);
        assert!(world.apply_emp_pulse_disable_field_at(1, Vec3::ZERO, None));
        assert!(world.host_object(ids[1]).unwrap().status.disabled_emp);
        assert_eq!(world.players[&1].radar_count, 0);
        assert_eq!(world.players[&1].power_produced, 0);
        let deadline = world.host_object(ids[0]).unwrap().status.disabled_emp_until_frame;
        assert!(deadline > world.frame);
        for id in ids {
            let status = &world.host_object(id).unwrap().status;
            assert!(status.disabled_emp);
            assert_eq!(status.disabled_emp_until_frame, deadline);
            assert!(!peer.host_object(id).unwrap().status.disabled_emp);
        }
        world.set_current_frame(u64::from(world.frame) + 1);
        assert!(world.apply_emp_pulse_disable_field_at(1, Vec3::ZERO, None));
        assert_eq!(world.players[&1].radar_count, 0);
        assert_eq!(world.players[&1].power_produced, 0);
        assert_eq!(peer.players[&1].radar_count, 1);
        assert_eq!(peer.players[&1].power_produced, 20);
    }

    #[test]
    fn spawned_hacked_and_paralyzed_deadlines_can_shorten_and_restore_the_grid() {
        for (disable, deadline) in [
            (
                GameLogic::apply_owned_hacked_disabled as fn(&mut GameLogic, ObjectId, u32) -> bool,
                (|object: &Object| object.status.disabled_hacked_until_frame) as fn(&Object) -> u32,
            ),
            (
                GameLogic::apply_owned_paralyzed_disabled,
                |object: &Object| object.status.disabled_paralyzed_until_frame,
            ),
        ] {
            let (mut world, ids) = spawned_radar_world();
            assert!(disable(&mut world, ids[0], 100));
            assert!(disable(&mut world, ids[0], 80));
            assert_eq!(world.players[&1].radar_count, 0);
            assert_eq!(world.players[&1].power_produced, 0);
            for id in ids {
                let object = world.host_object(id).unwrap();
                assert!(object.has_disabled_type());
                assert_eq!(deadline(object), 80);
            }
            world.set_current_frame(79);
            world.expire_owned_disabled_statuses();
            assert_eq!(world.players[&1].power_produced, 0);
            world.set_current_frame(80);
            world.expire_owned_disabled_statuses();
            assert_eq!(world.players[&1].radar_count, 1);
            assert_eq!(world.players[&1].power_produced, 20);
            for id in ids {
                assert!(!world.host_object(id).unwrap().has_disabled_type());
            }
        }
    }

    #[test]
    fn ai_tick_does_not_overwrite_a_spawn_childs_independent_hack_deadline() {
        let (mut world, [parent, child, grandchild]) = spawned_radar_world();
        world.set_current_frame(10);
        assert!(world.apply_owned_hacked_disabled(parent, 80));
        assert!(world.apply_owned_hacked_disabled(child, 30));
        world.update_ai(&[parent, child, grandchild], LOGIC_FRAME_TIMESTEP).unwrap();

        assert_eq!(world.objects[&parent].status.disabled_hacked_until_frame, 80);
        for id in [child, grandchild] {
            assert_eq!(world.objects[&id].status.disabled_hacked_until_frame, 30);
        }
        world.set_current_frame(30);
        world.expire_owned_disabled_statuses();
        assert!(world.objects[&parent].status.disabled_hacked);
        assert!(!world.objects[&child].status.disabled_hacked);
        assert_eq!(world.players[&1].power_produced, 20);
        assert_eq!(world.players[&1].radar_count, 1);
    }

    #[test]
    fn named_held_propagates_to_spawn_children_and_preserves_their_callback_effects() {
        let (mut world, ids) = spawned_radar_world();
        let [parent, child, _] = ids;
        world.set_owned_named_held(parent, true);
        assert!(ids.iter().all(|id| world.objects[id].is_held_disabled()));
        assert_eq!(world.players[&1].radar_count, 0);
        assert_eq!(world.players[&1].power_produced, 0);

        world.set_owned_named_held(parent, false);
        assert!(ids.iter().all(|id| !world.objects[id].has_disabled_type()));
        assert_eq!(world.players[&1].radar_count, 0);
        world.update_player_resources(0.);
        assert_eq!(world.players[&1].power_produced, 0);

        world.set_owned_named_held(child, true);
        world.set_owned_named_held(parent, false);
        assert!(
            world.objects[&child].is_held_disabled(),
            "an unchanged parent deadline does not revisit its children"
        );
    }

    #[test]
    fn held_deadline_expires_in_original_order_after_save_and_load() {
        use game_engine::common::game_common::FOREVER;

        let (mut world, ids) = spawned_radar_world();
        let (mut restored, restored_ids) = spawned_radar_world();
        assert_eq!(ids, restored_ids);
        let [parent, plant, _] = ids;
        world.set_current_frame(u64::from(FOREVER) - 1);
        world.set_owned_named_held(parent, true);
        assert!(world.apply_owned_paralyzed_disabled(plant, FOREVER));
        assert!(ids.iter().all(|id| world.objects[id].is_held_disabled()));
        assert_eq!(world.players[&1].power_produced, 0);

        let builder = crate::save_load::snapshot::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&world).unwrap();
        let bytes = bincode_legacy::serialize(&snapshot).unwrap();
        let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
        builder.restore_from_snapshot(&decoded, &mut restored).unwrap();

        for (owner, credited_base) in [(&mut world, 0), (&mut restored, 20)] {
            owner.expire_owned_disabled_statuses();
            assert!(ids.iter().all(|id| owner.objects[id].is_held_disabled()));
            assert_eq!(owner.players[&1].power_produced, credited_base);
            owner.set_current_frame(u64::from(FOREVER));
            owner.expire_owned_disabled_statuses();
            assert!(ids.iter().all(|id| !owner.objects[id].has_disabled_type()));
            // HELD clears before PARALYZED, whose recursive recovery is
            // followed by the suspended HELD recovery on the same object.
            assert_eq!(owner.players[&1].power_produced, credited_base + 40);
            assert_eq!(owner.players[&1].radar_count, 2);
        }
    }

    #[test]
    fn saved_inactive_held_deadline_still_propagates_a_named_release() {
        use game_engine::common::game_common::FOREVER;

        let (mut world, ids) = spawned_radar_world();
        let (mut restored, restored_ids) = spawned_radar_world();
        assert_eq!(ids, restored_ids);
        let [parent, _, grandchild] = ids;
        world.set_current_frame(u64::from(FOREVER));
        world.set_owned_named_held(parent, true);
        assert!(ids.iter().all(|id| !world.objects[id].is_held_disabled()));
        world
            .objects
            .get_mut(&grandchild)
            .unwrap()
            .set_status_disabled_held(true);

        let builder = crate::save_load::snapshot::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&world).unwrap();
        let bytes = bincode_legacy::serialize(&snapshot).unwrap();
        let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
        builder.restore_from_snapshot(&decoded, &mut restored).unwrap();

        assert!(restored.objects[&grandchild].is_held_disabled());
        restored.set_owned_named_held(parent, false);
        assert!(ids.iter().all(|id| !restored.objects[id].is_held_disabled()));
        assert!(world.objects[&grandchild].is_held_disabled());
    }

    #[test]
    fn same_frame_disable_expiry_delivers_each_recursive_recovery_callback() {
        let (mut world, ids) = spawned_radar_world();
        let (mut restored, restored_ids) = spawned_radar_world();
        let (mut peer, peer_ids) = spawned_radar_world();
        assert_eq!(ids, restored_ids);
        assert_eq!(ids, peer_ids);
        let [_, plant, spawned] = ids;

        assert!(world.apply_owned_hacked_disabled(plant, 6));
        assert!(world.apply_owned_emp_disabled(plant, 6));
        assert!(peer.apply_owned_emp_disabled(plant, 6));
        assert_eq!(world.players[&1].power_produced, 0);
        assert_eq!(world.players[&1].radar_count, 0);

        let builder = crate::save_load::snapshot::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&world).unwrap();
        let bytes = bincode_legacy::serialize(&snapshot).unwrap();
        let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
        builder.restore_from_snapshot(&decoded, &mut restored).unwrap();

        for (owner, credited_base) in [(&mut world, 0), (&mut restored, 20)] {
            owner.set_current_frame(5);
            owner.expire_owned_disabled_statuses();
            assert_eq!(owner.players[&1].power_produced, credited_base);
            assert_eq!(owner.players[&1].radar_count, 0);
            owner.set_current_frame(6);
            owner.expire_owned_disabled_statuses();
            for id in [plant, spawned] {
                assert!(!owner.objects[&id].has_disabled_type());
            }
            // Clear HACKED -> checkDisabledStatus -> clear EMP -> enable,
            // then the suspended HACKED clear observes the empty mask too.
            assert_eq!(owner.players[&1].power_produced, credited_base + 40);
            assert_eq!(owner.players[&1].radar_count, 2);
            owner.update_player_resources(0.);
            owner.expire_owned_disabled_statuses();
            assert_eq!(owner.players[&1].power_produced, credited_base + 40);
            assert_eq!(owner.players[&1].radar_count, 2);
        }

        peer.set_current_frame(6);
        peer.expire_owned_disabled_statuses();
        assert_eq!(peer.players[&1].power_produced, 20);
        assert_eq!(peer.players[&1].radar_count, 1);
    }

    #[test]
    fn spawn_parent_expiry_clears_the_same_effect_before_longer_child_deadline() {
        let (mut world, [parent, child, grandchild]) = spawned_radar_world();
        assert!(world.apply_owned_emp_disabled(parent, 100));
        assert!(world.apply_owned_emp_disabled(child, 150));
        assert!(world.apply_owned_hacked_disabled(grandchild, 180));
        assert_eq!(world.players[&1].power_produced, 0);
        assert_eq!(world.players[&1].radar_count, 0);

        let builder = crate::save_load::snapshot::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&world).unwrap();
        let bytes = bincode_legacy::serialize(&snapshot).unwrap();
        let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
        let (mut restored, ids) = spawned_radar_world();
        assert_eq!(ids, [parent, child, grandchild]);
        builder.restore_from_snapshot(&decoded, &mut restored).unwrap();

        for (receiver, credited_base) in [(&mut world, 0), (&mut restored, 20)] {
            receiver.set_current_frame(99);
            receiver.expire_owned_disabled_statuses();
            assert!(receiver.host_object(child).unwrap().status.disabled_emp);
            assert_eq!(receiver.players[&1].power_produced, credited_base);

            receiver.set_current_frame(100);
            receiver.expire_owned_disabled_statuses();
            for id in [parent, child, grandchild] {
                assert!(!receiver.host_object(id).unwrap().status.disabled_emp);
            }
            assert!(receiver.host_object(grandchild).unwrap().status.disabled_hacked);
            assert_eq!(receiver.players[&1].power_produced, credited_base + 20);
            assert_eq!(receiver.players[&1].radar_count, 1);
        }
    }

    fn power_scaffold_world() -> (GameLogic, ObjectId, ObjectId) {
        let mut world = GameLogic::new();
        let source = "
Object PeriodicPlant
  KindOf = STRUCTURE
  EnergyProduction = 20
  Body = ActiveBody BodyTag
    MaxHealth = 100
    InitialHealth = 100
    SubdualDamageCap = 200
    SubdualDamageHealRate = 100
    SubdualDamageHealAmount = 150
  End
  Behavior = DestroyDie DeathTag
  End
End
Object PeriodicPowered
  KindOf = STRUCTURE POWERED
  EnergyProduction = -10
End";
        world
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "periodic-energy.ini".into(),
                    source.into(),
                )])
                .unwrap(),
            )
            .unwrap();
        let mut player = Player::new(1, Team::USA, "PeriodicEnergy", true);
        player.map_side.map_player_name = "PeriodicEnergy".into();
        player.add_radar(false);
        world.add_player(player);
        let plant = world
            .create_object_for_player("PeriodicPlant", 1, Vec3::ZERO)
            .unwrap();
        let scaffold = world
            .create_object_under_construction_for_player("PeriodicPowered", 1, Vec3::ZERO, 0.)
            .unwrap().expect("ObjectId");
        (world, plant, scaffold)
    }

    #[test]
    fn physical_retirement_delivers_the_resident_players_energy_change_once() {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::host_usa_pilot::HostDeathType;
        use crate::game_logic::object::DamageHitContext;

        for lethal_damage in [false, true] {
            let (mut world, plant, scaffold) = power_scaffold_world();
            let (peer, peer_plant, peer_scaffold) = power_scaffold_world();
            assert_eq!((plant, scaffold), (peer_plant, peer_scaffold));
            let consumer = world
                .create_object_for_player("PeriodicPowered", 1, Vec3::X)
                .unwrap();
            assert_eq!(world.players[&1].power_available, 10);

            if lethal_damage {
                world
                    .apply_owned_damage(
                        plant,
                        150.,
                        None,
                        DamageType::Unresistable,
                        HostDeathType::Normal,
                        None,
                        &DamageHitContext::default(),
                    )
                    .unwrap();
            } else {
                world.destroy_object(plant).unwrap();
            }
            assert!(world.objects.contains_key(&plant));
            assert_eq!(world.players[&1].power_produced, 20);
            world.update_player_resources(0.);
            assert_eq!(
                world.players[&1].power_produced, 20,
                "onDelete keeps the Team contribution installed"
            );

            world.process_destroy_list();
            assert!(!world.objects.contains_key(&plant));
            assert_eq!(
                world.players[&1].power_produced, 0,
                "team removal applies the delta"
            );
            assert_eq!(world.players[&1].power_available, -10);
            assert!(!world.players[&1].has_radar());
            assert!(world.objects[&scaffold].status.disabled_underpowered);
            world.economy_events.complete_boundary();
            let receipts = world.economy_events.take_completed();
            assert_eq!(receipts.last().unwrap().power_available, -10);

            world.destroy_object(consumer).unwrap();
            world.process_destroy_list();
            world.process_destroy_list();
            assert_eq!(
                world.players[&1].power_consumed, 0,
                "a disabled consumer leaves once"
            );
            assert_eq!(world.players[&1].power_available, 0);
            assert!(world.players[&1].has_radar());
            assert!(!world.objects[&scaffold].status.disabled_underpowered);

            assert!(peer.players[&1].has_radar());
            assert_eq!(peer.players[&1].power_produced, 20);
            assert!(!peer.objects[&scaffold].status.disabled_underpowered);
        }

        let (mut world, plant, _) = power_scaffold_world();
        let unfinished = world
            .create_object_under_construction_for_player("PeriodicPlant", 1, Vec3::X, 0.)
            .unwrap()
            .unwrap();
        world.destroy_object(unfinished).unwrap();
        world.process_destroy_list();
        assert_eq!(world.players[&1].power_produced, 20);
        assert!(world.apply_owned_emp_disabled(plant, 100));
        assert_eq!(world.players[&1].power_produced, 0);
        world.destroy_object(plant).unwrap();
        world.process_destroy_list();
        assert_eq!(
            world.players[&1].power_produced, 0,
            "EMP already removed this output"
        );
    }

    fn receiving_energy_defaults_world() -> GameLogic {
        let source = "
Object DefaultGridPowerPlant
  KindOf = STRUCTURE
End
Object DefaultGridRelay
  KindOf = STRUCTURE
End
Object DefaultGridParent
  KindOf = STRUCTURE
  EnergyProduction = -4suffix
End
ChildObject DefaultGridChild DefaultGridParent
End";
        let mut world = GameLogic::new();
        world
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "energy-defaults.ini".into(),
                    source.into(),
                )])
                .unwrap(),
            )
            .unwrap();
        world.add_player(Player::new(1, Team::USA, "GridDefaults", true));
        world
    }

    #[test]
    fn receiving_energy_defaults_survive_map_layers_and_legacy_save_mirrors() {
        use crate::save_load::snapshot::ObjectTypeSnapshot;

        let mut world = receiving_energy_defaults_world();
        let mut peer = receiving_energy_defaults_world();
        let map = "Object DefaultGridRelay\n  EnergyProduction = -9\nEnd".to_owned();
        world.apply_receiving_module_overrides(std::slice::from_ref(&map)).unwrap();
        world.apply_receiving_module_overrides(&[]).unwrap();
        peer.apply_receiving_module_overrides(&[map]).unwrap();

        let spawn_grid = |world: &mut GameLogic| {
            ["DefaultGridPowerPlant", "DefaultGridRelay", "DefaultGridChild"]
                .map(|name| world.create_object_for_player(name, 1, Vec3::ZERO).unwrap())
        };
        let [plant, relay, child] = spawn_grid(&mut world);
        assert_eq!(spawn_grid(&mut peer), [plant, relay, child]);
        assert_eq!(world.objects[&plant].power_provided, 0, "omitted producer base");
        assert_eq!(world.objects[&relay].power_consumed, 0, "omitted consumer base");
        assert_eq!(world.objects[&child].power_consumed, 4, "inherited signed prefix");
        assert_eq!(world.players[&1].power_consumed, 4);
        assert_eq!(peer.players[&1].power_consumed, 13);
        world.update_player_resources(0.);
        assert_eq!(world.players[&1].power_produced, 0);
        assert_eq!(world.players[&1].power_consumed, 4);

        let builder = crate::save_load::snapshot::SnapshotBuilder::new();
        let mut snapshot = builder.create_world_snapshot(&world).unwrap();
        for (id, provided, required) in [(plant, 10, 0), (relay, 0, -3)] {
            let ObjectTypeSnapshot::Building(saved) =
                &mut snapshot.objects.get_mut(&id).unwrap().object_type
            else {
                panic!("expected saved structure {id:?}");
            };
            saved.power_provided = provided;
            saved.power_required = required;
        }
        let bytes = bincode_legacy::serialize(&snapshot).unwrap();
        let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
        let mut restored = receiving_energy_defaults_world();
        builder.restore_from_snapshot(&decoded, &mut restored).unwrap();

        assert_eq!(restored.objects[&plant].power_provided, 0);
        assert_eq!(restored.objects[&relay].power_consumed, 0);
        assert_eq!(restored.players[&1].power_produced, 0);
        assert_eq!(restored.players[&1].power_consumed, 4);
        assert_eq!(peer.players[&1].power_consumed, 13);
    }

    #[test]
    fn instant_power_plant_completion_restores_grid_while_construction_flag_remains_set() {
        let (mut world, plant, scaffold) = power_scaffold_world();
        let (peer, peer_plant, peer_scaffold) = power_scaffold_world();
        assert_eq!((plant, scaffold), (peer_plant, peer_scaffold));
        let consumer = world
            .create_object_for_player("PeriodicPowered", 1, Vec3::X)
            .unwrap();
        world.set_current_frame(5);
        assert!(world.apply_owned_emp_disabled(plant, 30));
        assert_eq!(world.players[&1].power_available, -10);
        assert!(!world.players[&1].has_radar());
        assert!(world.objects[&consumer].status.disabled_underpowered);

        let instant = world
            .create_instant_line_build_object_for_player(
                "PeriodicPlant",
                1,
                Vec3::new(40., 0., 50.),
                0.,
                plant,
            )
            .unwrap();
        assert!(world.objects[&instant].status.under_construction);
        assert!(world.objects[&instant].construction_power_admitted);
        assert_eq!(world.players[&1].power_produced, 20, "Player completion");
        assert_eq!(world.players[&1].power_available, 10);
        assert!(world.players[&1].has_radar());
        assert!(!world.objects[&consumer].status.disabled_underpowered);

        let builder = crate::save_load::snapshot::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&world).unwrap();
        let bytes = bincode_legacy::serialize(&snapshot).unwrap();
        let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
        let (mut restored, _, _) = power_scaffold_world();
        builder.restore_from_snapshot(&decoded, &mut restored).unwrap();

        for owner in [&mut world, &mut restored] {
            assert!(owner.objects[&instant].status.under_construction);
            assert!(owner.objects[&instant].construction_power_admitted);
            assert_eq!(owner.players[&1].power_produced, 20);
            owner.update_player_resources(0.);
            assert_eq!(owner.players[&1].power_produced, 20);
            assert_eq!(owner.players[&1].power_consumed, 10);
            assert!(owner.players[&1].has_radar());
            assert!(!owner.objects[&consumer].status.disabled_underpowered);
            assert!(owner.objects[&plant].status.disabled_emp);
            assert!(owner.apply_owned_emp_disabled(instant, 12));
            assert_eq!(owner.players[&1].power_produced, 0);
            assert!(!owner.players[&1].has_radar());
            owner.set_current_frame(12);
            owner.expire_owned_disabled_statuses();
            assert_eq!(owner.players[&1].power_produced, 20);
            assert!(owner.players[&1].has_radar());
            assert!(owner.objects[&instant].status.under_construction);
        }
        assert!(peer.players[&1].has_radar());
        assert_eq!(peer.players[&1].power_produced, 20);
        assert_eq!(peer.players[&1].power_consumed, 0);
    }

    #[test]
    fn subdual_damage_and_decay_update_power_without_a_grid_scan() {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::host_usa_pilot::HostDeathType;
        use crate::game_logic::object::DamageHitContext;

        for emp_overlaps_decay in [false, true] {
            let (mut world, plant, _) = power_scaffold_world();
            let consumer = world
                .create_object_for_player("PeriodicPowered", 1, Vec3::X)
                .unwrap();
            world.set_current_frame(5);
            let hit = |world: &mut GameLogic, amount| {
                world
                    .apply_owned_damage(
                        plant,
                        amount,
                        None,
                        DamageType::SubdualUnresistable,
                        HostDeathType::Normal,
                        None,
                        &DamageHitContext::default(),
                    )
                    .unwrap();
            };
            hit(&mut world, 60.);
            assert!(!world.objects[&plant].status.disabled_subdued);
            assert_eq!(world.players[&1].power_available, 10);
            hit(&mut world, 60.);
            assert!(world.objects[&plant].status.disabled_subdued);
            assert_eq!(world.objects[&plant].health.current, 100.);
            assert_eq!(world.players[&1].power_produced, 0, "threshold-crossing hit");
            assert_eq!(world.players[&1].power_available, -10);
            assert!(!world.players[&1].has_radar());
            assert!(world.objects[&consumer].status.disabled_underpowered);

            hit(&mut world, 20.);
            if emp_overlaps_decay {
                assert!(world.apply_owned_emp_disabled(plant, 30));
            }
            assert_eq!(world.players[&1].power_produced, 0);

            let builder = crate::save_load::snapshot::SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&world).unwrap();
            let bytes = bincode_legacy::serialize(&snapshot).unwrap();
            let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
            let (mut restored, _, _) = power_scaffold_world();
            builder.restore_from_snapshot(&decoded, &mut restored).unwrap();

            for (owner, credited_base) in [(&mut world, 0), (&mut restored, 20)] {
                assert_eq!(owner.players[&1].power_produced, credited_base);
                assert!(owner.objects[&plant].status.disabled_subdued);
                for frame in 6..=15 {
                    owner.set_current_frame(frame);
                    owner.update_ai(&[plant], LOGIC_FRAME_TIMESTEP).unwrap();
                    if !owner.objects[&plant].status.disabled_subdued {
                        break;
                    }
                    assert_eq!(owner.players[&1].power_produced, credited_base);
                }
                assert!(!owner.objects[&plant].status.disabled_subdued);
                assert_eq!(
                    owner.players[&1].power_produced,
                    credited_base + if emp_overlaps_decay { 0 } else { 20 },
                    "decay frame"
                );
                let brownout = emp_overlaps_decay && credited_base == 0;
                assert_eq!(owner.players[&1].has_radar(), !brownout);
                assert_eq!(
                    owner.objects[&consumer].status.disabled_underpowered,
                    brownout
                );
                if emp_overlaps_decay {
                    assert!(owner.objects[&plant].status.disabled_emp);
                    owner.set_current_frame(30);
                    owner.expire_owned_disabled_statuses();
                    assert_eq!(owner.players[&1].power_produced, credited_base + 20);
                    assert!(owner.players[&1].has_radar());
                    assert!(!owner.objects[&consumer].status.disabled_underpowered);
                }
            }
        }
    }

    #[test]
    fn recovering_producer_transfers_signed_energy_to_consumption() {
        let fixture = || {
            let mut world = GameLogic::new();
            let source = "
Object SignedExpiryPlant
 KindOf = STRUCTURE
 EnergyProduction = 20
 EnergyBonus = -25
 Behavior = OverchargeBehavior Overcharge
  HealthPercentToDrainPerSecond = 0%
  NotAllowedWhenHealthBelowPercent = 0%
 End
End
Object SignedExpiryLoad
 KindOf = STRUCTURE POWERED
 EnergyProduction = -10
End";
            world
                .admit_receiving_module_content(
                    crate::assets::ReceivingObjectContent::from_sources([(
                        "signed-expiry.ini".into(),
                        source.into(),
                    )])
                    .unwrap(),
                )
                .unwrap();
            world.add_player(Player::new(1, Team::USA, "SignedExpiry", true));
            let plant = world
                .create_object_for_player("SignedExpiryPlant", 1, Vec3::ZERO)
                .unwrap();
            world
                .create_object_for_player("SignedExpiryLoad", 1, Vec3::X)
                .unwrap();
            (world, plant)
        };
        let energy = |world: &GameLogic| {
            let player = &world.players[&1];
            (
                player.power_produced,
                player.power_consumed,
                player.power_available,
            )
        };
        let (mut world, plant) = fixture();
        let (mut restored, peer_plant) = fixture();
        assert_eq!(plant, peer_plant);

        world.set_current_frame(4);
        assert!(world.apply_owned_emp_disabled(plant, 6));
        assert!(world.toggle_overcharge_object(plant));
        assert_eq!(world.objects[&plant].power_provided, -5);
        assert_eq!(energy(&world), (-25, 10, -35));
        assert_eq!(energy(&restored), (20, 10, 10));

        let builder = crate::save_load::snapshot::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&world).unwrap();
        let bytes = bincode_legacy::serialize(&snapshot).unwrap();
        let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
        builder.restore_from_snapshot(&decoded, &mut restored).unwrap();

        // The release C++ Energy path reconstructs the base 20 during Team
        // transfer, before the saved EMP mask, then reapplies the signed bonus.
        for (owner, production) in [(&mut world, -25), (&mut restored, -5)] {
            assert_eq!(energy(owner), (production, 10, production - 10));
            owner.set_current_frame(5);
            owner.expire_owned_disabled_statuses();
            assert_eq!(energy(owner), (production, 10, production - 10));
            owner.set_current_frame(6);
            owner.expire_owned_disabled_statuses();
            assert!(!owner.objects[&plant].status.disabled_emp);
            assert_eq!(
                energy(owner),
                (production, 15, production - 15),
                "C++ Energy::adjustPower(-5, true)"
            );
            owner.update_player_resources(0.);
            assert_eq!(energy(owner), (production, 15, production - 15));

            assert!(owner.apply_owned_hacked_disabled(plant, 8));
            assert_eq!(energy(owner), (production, 10, production - 10));
            owner.update_player_resources(0.);
            assert_eq!(energy(owner), (production, 10, production - 10));
        }
    }

    #[test]
    fn recovering_emp_producer_preserves_a_powered_scaffolds_sabotage_brownout() {
        let (mut world, plant, scaffold) = power_scaffold_world();
        let (mut peer, peer_plant, peer_scaffold) = power_scaffold_world();
        assert_eq!((plant, scaffold), (peer_plant, peer_scaffold));

        world.set_current_frame(4);
        world.sabotage_player_power(1, 10);
        assert!(world.apply_owned_emp_disabled(plant, 6));
        assert_eq!(world.players[&1].power_produced, 0);
        assert!(world.objects[&scaffold].status.disabled_underpowered);

        let builder = crate::save_load::snapshot::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&world).unwrap();
        let bytes = bincode_legacy::serialize(&snapshot).unwrap();
        let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
        let (mut restored, _, _) = power_scaffold_world();
        builder.restore_from_snapshot(&decoded, &mut restored).unwrap();

        for (owner, credited_base) in [(&mut world, 0), (&mut restored, 20)] {
            owner.set_current_frame(6);
            owner.players.get_mut(&1).unwrap().power_available = 999;
            owner.expire_owned_disabled_statuses();
            assert!(!owner.objects[&plant].status.disabled_emp);
            assert_eq!(owner.players[&1].power_produced, credited_base + 20);
            assert_eq!(owner.players[&1].energy_production_at(6), 0);
            assert_eq!(owner.players[&1].power_available, 0);
            assert!(!owner.players[&1].has_radar());
            assert!(owner.objects[&scaffold].status.under_construction);
            assert!(
                owner.objects[&scaffold].status.disabled_underpowered,
                "an EMP expiry still signals the current sabotage brownout to POWERED scaffolds"
            );

            owner.set_current_frame(11);
            owner.expire_player_power_sabotage(1);
            assert!(!owner.objects[&scaffold].status.disabled_underpowered);
            assert_eq!(owner.players[&1].power_produced, credited_base + 20);
        }
        assert!(peer.players[&1].has_radar());
        assert_eq!(peer.players[&1].power_produced, 20);
        assert!(!peer.objects[&scaffold].status.disabled_underpowered);

        // At deadline equality an actual producer signal can restore radar
        // and POWERED actors even before Player::update clears the timer.
        let retiring_plant = peer
            .create_object_for_player("PeriodicPlant", 1, Vec3::X)
            .unwrap();
        peer.set_current_frame(4);
        peer.sabotage_player_power(1, 6);
        assert!(peer.apply_owned_emp_disabled(peer_plant, 6));
        peer.set_current_frame(6);
        peer.update_player_resources(0.);
        // Mirrors late deletion after the resource phase of an ordinary tick.
        peer.destroy_object(retiring_plant).unwrap();
        peer.process_destroy_list();
        assert!(peer.host_object(retiring_plant).is_none());
        peer.expire_owned_disabled_statuses();
        assert_eq!(peer.players[&1].power_sabotaged_till_frame, 6);
        assert_eq!(peer.players[&1].power_produced, 20);
        assert!(peer.players[&1].has_radar());
        assert!(!peer.objects[&peer_scaffold].status.disabled_underpowered);
    }

    #[test]
    fn periodic_sabotage_expiry_uses_same_callback_and_includes_powered_scaffolds() {
        let (mut world, _, scaffold) = power_scaffold_world();
        assert!(
            world
                .host_object(scaffold)
                .unwrap()
                .status
                .under_construction
        );
        assert_eq!(
            (
                world.get_player(1).unwrap().power_produced,
                world.get_player(1).unwrap().power_consumed
            ),
            (20, 0)
        );
        world.frame = 4;
        world.sabotage_player_power(1, 5);
        assert!(
            world
                .host_object(scaffold)
                .unwrap()
                .status
                .disabled_underpowered,
            "CPP doPowerDisable has no construction exclusion"
        );
        assert!(!world.get_player(1).unwrap().has_radar());
        world.frame = 5;
        world.update_player_resources(0.);
        world.update_power_disabled_state();
        assert!(
            world
                .host_object(scaffold)
                .unwrap()
                .status
                .disabled_underpowered
        );
        assert!(
            !world.get_player(1).unwrap().has_radar(),
            "the Player expiry callback has yet to run at deadline equality"
        );
        assert_eq!(
            (
                world.get_player(1).unwrap().energy_production_at(5),
                world.get_player(1).unwrap().power_consumed
            ),
            (20, 0)
        );
        crate::ai::AIManager::update_owned(&mut world, 0.);
        assert_eq!(world.get_player(1).unwrap().power_sabotaged_till_frame, 5);
        assert!(!world.get_player(1).unwrap().has_radar());
        world.frame = 6;
        crate::ai::AIManager::update_owned(&mut world, 0.);
        assert_eq!(world.get_player(1).unwrap().power_sabotaged_till_frame, 0);
        assert!(world.get_player(1).unwrap().has_radar());
        assert!(!world.host_object(scaffold).unwrap().status.disabled_underpowered);
    }
    #[test]
    fn null_controller_keeps_factory_team_and_grid_independent_of_later_same_faction_player() {
        let mut world = GameLogic::new();
        world.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([(
            "unowned-energy.ini".into(),
            "Object UnownedConsumer\nKindOf = STRUCTURE POWERED\nEnergyProduction = -10\nEnd\n".into(),
        )]).unwrap()).unwrap();
        let unowned = world
            .create_object("UnownedConsumer", Team::USA, Vec3::ZERO)
            .unwrap();
        let object = world.host_object(unowned).unwrap();
        assert_eq!(object.owner_player_id, None);
        assert!(object.team_instance_name.is_empty());
        assert_eq!(object.team_instance_id, None);
        let mut player = Player::new(1, Team::USA, "LaterOwner", true);
        player.map_side.map_player_name = "LaterOwner".into();
        world.add_player(player);
        let owned = world
            .create_object_for_player("UnownedConsumer", 1, Vec3::ONE)
            .unwrap();
        world.update_player_resources(0.);
        world.update_power_disabled_state();
        assert_eq!(
            world.players[&1].power_consumed, 10,
            "NULL-owned actor cannot join the later faction grid"
        );
        assert!(
            world
                .host_object(owned)
                .unwrap()
                .status
                .disabled_underpowered
        );
        assert!(
            !world
                .host_object(unowned)
                .unwrap()
                .status
                .disabled_underpowered
        );
        let mut invalid = Object::new(
            world.host_object(unowned).unwrap().thing().template.clone(),
            ObjectId(900),
            Team::USA,
        );
        invalid.owner_player_id = Some(99);
        assert!(world.try_add_object(invalid).is_err());
        assert!(world.host_object(ObjectId(900)).is_none());
        assert_eq!(world.players[&1].power_consumed, 10);
    }
}
