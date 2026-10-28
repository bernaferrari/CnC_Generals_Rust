// Immutable admission data for the queued Area owner. The legacy standalone
// resolver stays unchanged. Keep these calculations aligned with its Area arm.
// C++ Weapon.cpp:1283; PartitionManager.cpp:3585–3602; SimpleObjectIterator.cpp:51–80.
pub(in crate::game_logic) struct AreaDamage<'a> {
    event: &'a DamageEvent,
    shooter_pos: Option<Vec3>,
    shooter_producer: Option<ObjectId>,
    shooter_dir_xz: Option<(f32, f32)>,
    source_is_defector: bool,
    radius_damage_angle: f32,
    position: Vec3,
    primary_victim: Option<ObjectId>,
}
impl<'a> AreaDamage<'a> {
    pub(in crate::game_logic) fn new(
        event: &'a DamageEvent,
        objects: HostObjectRead<'_>,
        radius_damage_angle: f32,
        damage_at_self: bool,
    ) -> Option<Self> {
        let DamageEvent::Area {
            shooter_id,
            position,
            primary_victim,
            radius_source_present,
            radius_source_id,
            radius_source_producer,
            radius_source_pose,
            ..
        } = event
        else {
            return None;
        };
        let source = objects.lookup(shooter_id);
        Some(Self {
            event,
            shooter_pos: radius_source_pose
                .map(|(position, _)| position)
                .or_else(|| source.map(|o| o.get_position())),
            shooter_producer: *radius_source_producer,
            shooter_dir_xz: radius_source_pose
                .map(|(_, direction)| direction)
                .or_else(|| source.map(|o| o.unit_direction_xz())),
            // Moving projectiles are distinct from their launcher. Only a
            // projectileless source uses the launcher's current Object state.
            source_is_defector: *radius_source_present
                && radius_source_id == shooter_id
                && source.is_some_and(Object::is_undetected_defector),
            radius_damage_angle,
            // A projectile detonation is itself the source. Projectileless
            // shots already carry the fire-time damage position and ID.
            position: if damage_at_self {
                radius_source_pose.map_or(*position, |(position, _)| position)
            } else {
                *position
            },
            primary_victim: if damage_at_self && radius_source_pose.is_some() {
                None
            } else {
                *primary_victim
            },
        })
    }
    /// Freeze candidate distances in this world's current HashMap encounter order.
    /// Reentrant victim callbacks must not change another candidate's damage band.
    pub(in crate::game_logic) fn candidates(
        &self,
        objects: HostObjectRead<'_>,
    ) -> Vec<(ObjectId, f32)> {
        let DamageEvent::Area {
            radius,
            secondary_radius,
            secondary_damage,
            ..
        } = self.event
        else {
            unreachable!()
        };
        let secondary_r = (*secondary_radius).max(0.0);
        let dual = secondary_r > *radius + 1e-3 && *secondary_damage > 0.0;
        let outer = if dual { secondary_r } else { *radius };
        objects
            .registered()
            .filter_map(|(id, obj)| {
                let distance = splash_from_bounding_sphere_3d(
                    self.position,
                    obj.get_position(),
                    victim_splash_sphere_radius(obj),
                );
                if distance > outer {
                    None
                } else {
                    Some((*id, distance))
                }
            })
            .collect()
    }
    /// Remaining admission is live per victim; source pose/producer and event
    /// ownership/template metadata retain the existing event-entry semantics.
    /// This deliberately does not redesign the residual C++ live-source mismatch.
    pub(in crate::game_logic) fn impact(
        &self,
        vid: ObjectId,
        obj: &Object,
        dist: f32,
        players: &HashMap<u32, crate::game_logic::Player>,
        team_factory: &gamelogic::team::TeamFactoryHandle,
        templates: &HashMap<String, crate::game_logic::ThingTemplate>,
    ) -> Option<(f32, Option<Vec3>)> {
        let DamageEvent::Area {
            position: _,
            damage,
            damage_type: _,
            death_type: _,
            radius,
            secondary_damage,
            secondary_radius,
            shock_wave_amount,
            shock_wave_radius,
            shock_wave_taper_off,
            shooter_id,
            radius_damage_affects,
            radius_source_present,
            radius_source_id,
            radius_source_producer: _,
            radius_source_team_id,
            radius_source_pose: _,
            shooter_owner_player_id,
            shooter_team_instance_name,
            shooter_template,
            primary_victim: _,
            radius_weapon_name: _,
            shooter_team: _,
        } = self.event
        else {
            unreachable!()
        };
        let primary_r = *radius;
        let secondary_r = (*secondary_radius).max(0.0);
        let shooter_pos = self.shooter_pos;
        let shooter_producer = self.shooter_producer;
        let shooter_dir_xz = self.shooter_dir_xz;
        let op = obj.get_position();
        let is_primary = self.primary_victim == Some(vid);
        let kills_self = *radius_source_present
            && (*radius_damage_affects
                & crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_KILLS_SELF)
                != 0
            && vid == *radius_source_id;
        if *radius_source_present && !is_primary && !kills_self {
            let airborne = obj.is_significantly_above_terrain();
            use gamelogic::common::Relationship;
            let relationship = if obj.is_undetected_defector() {
                Relationship::Neutral
            } else if self.source_is_defector {
                Relationship::Allies
            } else {
                crate::game_logic::GameLogic::object_relationship_from_identities(
                    team_factory,
                    players,
                    obj.owner_player_id,
                    obj.team_instance_id,
                    &obj.team_instance_name,
                    *shooter_owner_player_id,
                    *radius_source_team_id,
                    shooter_team_instance_name,
                )
            };
            let doesnt_affect_similar = crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_DOESNT_AFFECT_SIMILAR;
            if (*radius_damage_affects & doesnt_affect_similar) != 0
                && crate::game_logic::weapon_bootstrap::receiving_splash_templates_equivalent(
                    templates,
                    shooter_template,
                    &obj.template_name,
                )
            {
                // Weapon.cpp checks source->getRelationship(victim) here;
                // the ALLIES/ENEMIES mask below uses the reverse direction.
                let source_relationship = if self.source_is_defector {
                    Relationship::Neutral
                } else if obj.is_undetected_defector() {
                    Relationship::Allies
                } else {
                    crate::game_logic::GameLogic::object_relationship_from_identities(
                        team_factory,
                        players,
                        *shooter_owner_player_id,
                        *radius_source_team_id,
                        shooter_team_instance_name,
                        obj.owner_player_id,
                        obj.team_instance_id,
                        &obj.team_instance_name,
                    )
                };
                if source_relationship == Relationship::Allies {
                    return None;
                }
            }
            let allowed = crate::game_logic::weapon_bootstrap::radius_damage_affects_victim(
                *radius_damage_affects & !doesnt_affect_similar,
                relationship,
                *radius_source_id,
                vid,
                shooter_producer,
                airborne,
                false,
            );
            if !allowed {
                return None;
            }
        }
        if !leftover_radius_damage_cone_allows(
            self.radius_damage_angle,
            shooter_pos,
            shooter_dir_xz,
            op,
        ) {
            return None;
        }
        let dual = secondary_r > primary_r + 1e-3 && *secondary_damage > 0.0;
        let outer = if dual { secondary_r } else { primary_r };
        let area_damage = if !(dist <= outer) {
            0.0
        } else if kills_self && !is_primary {
            HUGE_DAMAGE_AMOUNT
        } else if dist <= primary_r {
            *damage
        } else if secondary_r > 0.0 {
            *secondary_damage
        } else {
            0.0
        };
        // Capture the force before body callbacks; apply after owned completion.
        let shock_amt = (*shock_wave_amount).max(0.0);
        let shock_r = (*shock_wave_radius).max(0.0);
        let shock_taper = (*shock_wave_taper_off).clamp(0.0, 1.0);
        let shock = if shock_amt > 0.0 && shock_r > 0.0 {
            crate::game_logic::weapon_bootstrap::compute_shock_wave_force(
                shooter_pos.unwrap_or(self.position),
                op,
                shock_amt,
                shock_r,
                shock_taper,
            )
        } else {
            None
        };
        Some((area_damage, shock))
    }
}
