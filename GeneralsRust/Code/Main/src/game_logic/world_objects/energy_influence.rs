//! Immediate receiving Energy entry and synchronous brownout callbacks.
use super::super::*;
use crate::game_logic::object::TimedDisable;

impl GameLogic {
    /// EMPUpdate calls Object::setDisabledUntil on the installed target.
    pub(in crate::game_logic) fn apply_owned_emp_disabled(
        &mut self,
        id: ObjectId,
        until: u32,
    ) -> bool {
        self.apply_owned_initial_disabled(id, until, TimedDisable::Emp)
    }

    /// SpecialAbilityUpdate calls Object::setDisabledUntil on its target.
    pub(in crate::game_logic) fn apply_owned_hacked_disabled(
        &mut self,
        id: ObjectId,
        until: u32,
    ) -> bool {
        self.apply_owned_initial_disabled(id, until, TimedDisable::Hacked)
    }

    /// BattlePlanUpdate calls Object::setDisabledUntil on the affected unit.
    pub(in crate::game_logic) fn apply_owned_paralyzed_disabled(
        &mut self,
        id: ObjectId,
        until: u32,
    ) -> bool {
        self.apply_owned_initial_disabled(id, until, TimedDisable::Paralyzed)
    }

    fn apply_owned_initial_disabled(
        &mut self,
        id: ObjectId,
        until: u32,
        effect: TimedDisable,
    ) -> bool {
        let epoch = self.visual_world_epoch;
        let (birth, became_disabled, deadline_changed) = {
            let Some(object) = self.objects.get_mut(&id) else {
                return false;
            };
            let was_disabled = object.has_disabled_type();
            if effect.is_power_style() && !object.is_power_style_disabled() {
                object.queue_power_disable_misc_audio(true, &mut self.health_events);
            }
            let mut audio = crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events);
            let changed = object.begin_timed_disable(effect, until, &mut audio);
            (
                object.visual_object_generation,
                !was_disabled && object.has_disabled_type(),
                changed,
            )
        };
        if deadline_changed {
            // C++ ContainModuleInterface::friend_getRider precedes SpawnBehavior.
            if let Some(rider) = self.containment_disable_rider(id) {
                self.apply_owned_initial_disabled(rider, until, effect);
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
                self.apply_owned_spawn_children_disabled_until(id, birth, epoch, children, until, effect);
                if !self.spawn_identity_matches(id, birth, epoch) {
                    return true;
                }
            }
        }

        // Object::setDisabledUntil calls onDisabledEdge after the children.
        // An earlier queued edge could be drained by a child's brownout.
        let producer = if became_disabled {
            let object = self.objects.get_mut(&id).unwrap();
            object.on_disabled_edge(true);
            (object.thing().template.energy_production.unwrap_or(0) > 0)
                .then_some((object.owner_player_id, object.power_provided))
        } else {
            None
        };

        self.admit_host_radar_disabled_edges(id);
        if let Some((Some(owner), power)) = producer {
            let brownout = self
                .players
                .get_mut(&owner)
                .and_then(|player| player.remove_template_energy(power, self.frame));
            if let Some(brownout) = brownout {
                self.admit_player_energy_brownout(owner, brownout);
            }
        }
        true
    }

    fn containment_disable_rider(&self, owner: ObjectId) -> Option<ObjectId> {
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
        until: u32,
        effect: TimedDisable,
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
            if self.host_object(child_id).is_some_and(Object::has_ai_update_interface) {
                let _ = self.set_ai_state_decision_aware_with_effects(
                    child_id,
                    crate::game_logic::AIState::Idle,
                    &mut crate::game_logic::ScriptPresentationEffects::default(),
                );
                if !self.spawn_identity_matches(parent_id, parent_birth, epoch) {
                    return;
                }
                if !still_exact_child(self) {
                    continue;
                }
            }
            self.apply_owned_initial_disabled(child_id, until, effect);
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
            let owner = object.owner_player_id?;
            let energy = object.thing().template.energy_production.unwrap_or(0);
            if object.status.under_construction || (energy > 0 && object.is_disabled()) {
                return None;
            }
            self.players.contains_key(&owner).then_some((owner, energy))
        }) else {
            return;
        };
        let player = self.players.get_mut(&owner).unwrap();
        let brownout = if incoming {
            player.add_template_energy(energy, self.frame)
        } else {
            player.remove_template_energy(energy, self.frame)
        };
        if let Some(brownout) = brownout {
            self.admit_player_energy_brownout(owner, brownout);
        }
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
                let Some(object) = self.objects.get_mut(&id).filter(|object| {
                    object.owner_player_id == Some(owner)
                        && object.team_instance_id == Some(team_id)
                        && object.is_kind_of(KindOf::Powered)
                }) else {
                    continue;
                };
                let was_disabled = object.is_disabled();
                let was_power = object.status.disabled_underpowered;
                if brownout && !was_power && !object.is_power_style_disabled() {
                    object.queue_power_disable_misc_audio(true, &mut self.health_events);
                } else if !brownout
                    && was_power
                    && !(object.status.disabled_emp
                        || object.status.disabled_subdued
                        || object.status.disabled_hacked)
                {
                    object.queue_power_disable_misc_audio(false, &mut self.health_events);
                }
                object.set_status_disabled_underpowered(brownout);
                let now_disabled = object.is_disabled();
                let producer_edge = (was_disabled != now_disabled
                    && object.thing().template.energy_production.unwrap_or(0) > 0)
                    .then_some((object.power_provided, now_disabled));
                self.admit_host_radar_disabled_edges(id);
                // Object::onDisabledEdge immediately adjusts a positive producer,
                // which synchronously reenters this Player callback before the
                // outer member traversal continues. Consumers never feed back.
                if let Some((power, disabled)) = producer_edge {
                    let next = self.players.get_mut(&owner).and_then(|player| {
                        if disabled {
                            player.remove_template_energy(power, self.frame)
                        } else {
                            player.add_template_energy(power, self.frame)
                        }
                    });
                    if let Some(next) = next {
                        self.admit_player_energy_brownout(owner, next);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spawned_radar_world() -> (GameLogic, [ObjectId; 3]) {
        let source = r#"
Object DisableNest
 KindOf = STRUCTURE SPAWNS_ARE_THE_WEAPONS
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
"#;
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

        for receiver in [&mut world, &mut restored] {
            receiver.set_current_frame(99);
            receiver.expire_owned_disabled_statuses();
            assert!(receiver.host_object(child).unwrap().status.disabled_emp);
            assert_eq!(receiver.players[&1].power_produced, 0);

            receiver.set_current_frame(100);
            receiver.expire_owned_disabled_statuses();
            for id in [parent, child, grandchild] {
                assert!(!receiver.host_object(id).unwrap().status.disabled_emp);
            }
            assert!(receiver.host_object(grandchild).unwrap().status.disabled_hacked);
            assert_eq!(receiver.players[&1].power_produced, 20);
            assert_eq!(receiver.players[&1].radar_count, 1);
        }
    }

    #[test]
    fn periodic_sabotage_expiry_uses_same_callback_and_includes_powered_scaffolds() {
        let mut world = GameLogic::new();
        let source = "Object PeriodicPlant\nKindOf = STRUCTURE\nEnergyProduction = 20\nEnd\nObject PeriodicPowered\nKindOf = STRUCTURE POWERED\nEnergyProduction = -10\nEnd\n";
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
        world
            .create_object_for_player("PeriodicPlant", 1, Vec3::ZERO)
            .unwrap();
        let scaffold = world
            .create_object_under_construction_for_player("PeriodicPowered", 1, Vec3::ZERO, 0.)
            .unwrap().expect("ObjectId");
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
        world.get_player_mut(1).unwrap().power_sabotaged_till_frame = 5;
        world.frame = 4;
        world.update_player_resources(0.);
        world.update_power_disabled_state();
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
            !world
                .host_object(scaffold)
                .unwrap()
                .status
                .disabled_underpowered
        );
        assert!(
            world.get_player(1).unwrap().has_radar(),
            "deadline equality recovers through the same exact callback"
        );
        assert_eq!(
            (
                world.get_player(1).unwrap().energy_production_at(5),
                world.get_player(1).unwrap().power_consumed
            ),
            (20, 0)
        );
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
