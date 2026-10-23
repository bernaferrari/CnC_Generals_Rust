//! Immediate receiving Energy entry and synchronous brownout callbacks.
use super::super::*;

impl GameLogic {
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
            .unwrap();
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
