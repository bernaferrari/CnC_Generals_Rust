use super::super::super::*;
use crate::game_logic::combat::DamageType;
use crate::game_logic::host_neutron_shell::{
    HOST_NEUTRON_BLAST_RADIUS, NEUTRON_AFFECT_AIRBORNE, NEUTRON_SHELL_AUDIO,
    NeutronBlastDefinition, NeutronEffect, in_neutron_blast_radius_2d,
    is_legal_neutron_blast_target, neutron_effect_for_target,
};
use crate::game_logic::host_usa_pilot::HostDeathType;

impl GameLogic {
    pub fn honesty_neutron_shell_projectile_ok(&self) -> bool {
        self.neutron_shells_spawned > 0
    }

    fn kill_neutron_target(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        // NeutronBlastBehavior calls Object::kill() without an attacker.
        Ok(self
            .apply_owned_kill_with_effects(
                id,
                DamageType::Unresistable,
                HostDeathType::Normal,
                effects,
            )?
            .is_some_and(|result| result.destroyed()))
    }

    fn kill_neutron_passengers(
        &mut self,
        target: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<u32> {
        let Some(object) = self.objects.get(&target) else {
            return Ok(0);
        };
        let container_id = if object.is_overlord_style_container() {
            // OverlordContain delegates to the bunker, leaving the portable
            // payload itself out of the passenger-kill operation.
            let redirected = self.redirected_containment_owner(target);
            if redirected == target {
                return Ok(0);
            }
            redirected
        } else {
            target
        };
        let Some(container) = self.objects.get(&container_id) else {
            return Ok(0);
        };
        let container_birth = std::sync::Arc::downgrade(&container.visual_draw_identity_token);
        let kind = container.get_template().contain_module.kind;
        let tunnel = container.is_tunnel_network_style_container();
        let ids = if tunnel {
            self.tunnel_network
                .contained_for_player(container.tunnel_system_key())
        } else {
            container.contained_units()
        };
        let riders: Vec<_> = ids
            .into_iter()
            .filter_map(|id| {
                let rider = self.objects.get(&id)?;
                Some((
                    id,
                    std::sync::Arc::downgrade(&rider.visual_draw_identity_token),
                ))
            })
            .collect();

        let mut killed = 0u32;
        for (rider, birth) in riders {
            if !self.object_birth_matches(container_id, &container_birth) {
                break;
            }
            if !self.object_birth_matches(rider, &birth) {
                continue;
            }
            let unlinked = if tunnel {
                self.exit_tunnel_network_unit_with_effects(rider, container_id, effects)
            } else {
                if self.objects[&rider].contained_by != Some(container_id) {
                    continue;
                }
                match kind {
                    ContainModuleKind::RailedTransport => self
                        .railed_kill_contained_with_effects(container_id, rider, effects)
                        .map_err(gamelogic::GameLogicError::Configuration)?,
                    ContainModuleKind::RiderChange => {
                        self.unlink_owned_cycle_rider_for_kill(container_id, rider, effects)
                    }
                    ContainModuleKind::Parachute => {
                        self.remove_parachute_rider_owned(container_id, rider, effects)?;
                        true
                    }
                    _ => self.unlink_owned_passenger_for_kill(container_id, rider, effects),
                }
            };
            if !unlinked || !self.object_birth_matches(rider, &birth) {
                continue;
            }
            if let Some(object) = self.objects.get_mut(&rider) {
                if object.contained_by == Some(container_id) {
                    object.finish_containment_reference_with_partition_owner(
                        None,
                        false,
                        &mut self.partition_manager,
                    );
                }
            }
            if self.kill_neutron_target(rider, effects)? {
                killed = killed.saturating_add(1);
            }
        }
        Ok(killed)
    }

    /// Kill infantry and passengers, then disable or kill the vehicle.
    /// Returns (infantry_kills, vehicles_unmanned, vehicle_kills).
    pub fn apply_neutron_blast_at(
        &mut self,
        impact: glam::Vec3,
        caster_team: Team,
        caster_id: Option<ObjectId>,
        affect_allies: bool,
    ) -> gamelogic::GameLogicResult<(u32, u32, u32)> {
        let center = (impact.x, impact.z);
        let off_map = caster_id
            .and_then(|id| self.objects.get(&id))
            .map(|source| source.status.off_map);
        // The explicit-position adapter also serves fixtures without a
        // partition phase. Authored Die callbacks use the map's COI iterator.
        let candidates = self
            .objects
            .registered()
            .filter_map(|(&id, object)| {
                let pos = object.get_position();
                (Some(id) != caster_id
                    && object.is_alive()
                    && !(object.contained_by.is_some() && object.status.masked)
                    && off_map.is_none_or(|off_map| off_map == object.status.off_map)
                    && in_neutron_blast_radius_2d(
                        center,
                        (pos.x, pos.z),
                        HOST_NEUTRON_BLAST_RADIUS,
                    ))
                .then(|| {
                    (
                        id,
                        std::sync::Arc::downgrade(&object.visual_draw_identity_token),
                    )
                })
            })
            .collect();
        let result = self.dispatch_neutron_blast(
            caster_team,
            caster_id,
            NeutronBlastDefinition {
                radius: HOST_NEUTRON_BLAST_RADIUS,
                affect_airborne: NEUTRON_AFFECT_AIRBORNE,
                affect_allies,
            },
            candidates,
            &mut Default::default(),
        )?;
        let (infantry_kills, vehicles_unmanned, _) = result;
        self.neutron_shell_residual_blasts = self.neutron_shell_residual_blasts.saturating_add(1);
        self.neutron_shell_residual_infantry_kills = self
            .neutron_shell_residual_infantry_kills
            .saturating_add(infantry_kills);
        self.neutron_shell_residual_vehicles_unmanned = self
            .neutron_shell_residual_vehicles_unmanned
            .saturating_add(vehicles_unmanned);
        self.queue_audio_event(
            AudioEventRequest::new(NEUTRON_SHELL_AUDIO)
                .with_position(impact)
                .with_priority(155),
        );
        Ok(result)
    }

    /// Original Die callback. FireFX and weapon audio belong to their
    /// authored weapon, outside this target effect.
    pub(in crate::game_logic) fn run_authored_neutron_blast(
        &mut self,
        emitter: ObjectId,
        definition: NeutronBlastDefinition,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(source) = self.objects.get(&emitter) else {
            return Ok(());
        };
        let impact = source.get_position();
        let caster_team = source.team;
        let off_map = source.status.off_map;
        let center = (impact.x, impact.z);
        // Original FROM_CENTER_2D / ITER_FASTEST applies Alive and SameMapStatus
        // while building the iterator. Enclosed passengers aren't in this grid.
        let candidates = self
            .partition_manager
            .ids_in_radius_iterator_order(impact.x, impact.z, definition.radius, |raw| {
                let id = ObjectId(raw);
                id != emitter
                    && self.objects.get_registered(&id).is_some_and(|object| {
                        let pos = object.get_position();
                        !object.status.effectively_dead
                            && off_map == object.status.off_map
                            && in_neutron_blast_radius_2d(center, (pos.x, pos.z), definition.radius)
                    })
            })
            .into_iter()
            .filter_map(|raw| {
                let id = ObjectId(raw);
                let object = self.objects.get_registered(&id)?;
                Some((
                    id,
                    std::sync::Arc::downgrade(&object.visual_draw_identity_token),
                ))
            })
            .collect();
        self.dispatch_neutron_blast(caster_team, Some(emitter), definition, candidates, effects)?;
        Ok(())
    }

    fn dispatch_neutron_blast(
        &mut self,
        caster_team: Team,
        caster_id: Option<ObjectId>,
        definition: NeutronBlastDefinition,
        candidates: Vec<(ObjectId, std::sync::Weak<()>)>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<(u32, u32, u32)> {
        let source_birth = caster_id
            .and_then(|id| self.objects.get(&id))
            .map(|source| std::sync::Arc::downgrade(&source.visual_draw_identity_token));

        let (mut infantry_kills, mut vehicles_unmanned, mut vehicle_kills) = (0u32, 0u32, 0u32);
        for (id, birth) in candidates {
            if !self.object_birth_matches(id, &birth) {
                continue;
            }
            let object = &self.objects[&id];
            let allied = !definition.affect_allies
                && caster_id
                    .zip(source_birth.as_ref())
                    .filter(|(id, birth)| self.object_birth_matches(*id, birth))
                    .map_or(object.team == caster_team, |(id, _)| {
                        self.object_relationship(&self.objects[&id], object)
                            == gamelogic::common::Relationship::Allies
                    });
            let infantry = object.is_kind_of(KindOf::Infantry);
            let vehicle = object.is_kind_of(KindOf::Vehicle);
            let drone = object.is_kind_of(KindOf::Drone);
            if !is_legal_neutron_blast_target(
                true,
                infantry,
                vehicle,
                object.is_kind_of(KindOf::Aircraft),
                object.is_kind_of(KindOf::Structure),
                drone,
                object.status.airborne_target,
                definition.affect_airborne,
                allied,
                definition.affect_allies,
            ) {
                continue;
            }

            // The original infantry and vehicle tests are independent. The
            // contains callback runs between them for this same target.
            let vehicle_effect = neutron_effect_for_target(
                false,
                vehicle,
                drone,
                object.is_kind_of(KindOf::CliffJumper),
            );
            if infantry && self.kill_neutron_target(id, effects)? {
                infantry_kills = infantry_kills.saturating_add(1);
            }
            if !self.object_birth_matches(id, &birth) {
                continue;
            }
            infantry_kills =
                infantry_kills.saturating_add(self.kill_neutron_passengers(id, effects)?);
            if !self.object_birth_matches(id, &birth) {
                continue;
            }
            match vehicle_effect {
                NeutronEffect::KillVehicle => {
                    if self.kill_neutron_target(id, effects)? {
                        vehicle_kills = vehicle_kills.saturating_add(1);
                    }
                }
                NeutronEffect::UnmanVehicle => {
                    let object = self.objects.get_mut(&id).unwrap();
                    if !object.is_unmanned() {
                        object.status.unmanned_owner_team = Some(object.team);
                        object.status.unmanned_owner_player_id = object.owner_player_id;
                    }
                    self.set_owned_unmanned(id, effects)?;
                    if !self.object_birth_matches(id, &birth) {
                        continue;
                    }
                    if self.objects[&id].has_ai_update_interface() {
                        self.unit_command_ai_idle_owned_with_effects(id, effects)?;
                        if !self.object_birth_matches(id, &birth) {
                            continue;
                        }
                    }
                    self.deselect_drawable(id);
                    self.objects.get_mut(&id).unwrap().set_terrain_decal(
                        crate::game_logic::host_battlemaster::TERRAIN_DECAL_NONE,
                    );
                    self.transfer_unmanned_to_neutral_team(id, effects)?;
                    vehicles_unmanned = vehicles_unmanned.saturating_add(1);
                }
                NeutronEffect::None | NeutronEffect::KillInfantry => {}
            }
        }

        Ok((infantry_kills, vehicles_unmanned, vehicle_kills))
    }
}
