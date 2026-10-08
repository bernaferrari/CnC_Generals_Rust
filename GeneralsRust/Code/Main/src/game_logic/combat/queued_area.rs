// Immutable admission data for the queued Area owner. The legacy standalone
// resolver stays unchanged. Keep these calculations aligned with its Area arm.
// C++ Weapon.cpp:1283; PartitionManager.cpp:3585–3602; SimpleObjectIterator.cpp:51–80.
pub(in crate::game_logic) struct AreaDamage<'a> {
    event: &'a DamageEvent,
    shooter_pos: Option<Vec3>,
    shooter_producer: Option<ObjectId>,
    shooter_dir_xz: Option<(f32, f32)>,
}
impl<'a> AreaDamage<'a> {
    pub(in crate::game_logic) fn new(
        event: &'a DamageEvent,
        objects: &HashMap<ObjectId, Object>,
    ) -> Option<Self> {
        let DamageEvent::Area { shooter_id, .. } = event else {
            return None;
        };
        let source = objects.get(shooter_id);
        Some(Self {
            event,
            shooter_pos: source.map(|o| o.get_position()),
            shooter_producer: source.and_then(|o| o.producer_id),
            shooter_dir_xz: source.map(|o| o.unit_direction_xz()),
        })
    }
    /// Freeze candidate distances in this world's current HashMap encounter order.
    /// Reentrant victim callbacks must not change another candidate's damage band.
    pub(in crate::game_logic) fn candidates(
        &self,
        objects: &HashMap<ObjectId, Object>,
    ) -> Vec<(ObjectId, f32)> {
        let DamageEvent::Area {
            position,
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
            .iter()
            .filter_map(|(id, obj)| {
                let distance = splash_from_bounding_sphere_3d(
                    *position,
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
        players: Option<&HashMap<u32, crate::game_logic::Player>>,
        team_factory: Option<&gamelogic::team::TeamFactoryHandle>,
    ) -> Option<(f32, Option<Vec3>)> {
        let DamageEvent::Area {
            position,
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
            shooter_owner_player_id,
            shooter_team_instance_name,
            shooter_template,
            primary_victim,
            radius_damage_angle,
            shooter_team,
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
        let is_primary = *primary_victim == Some(vid);
        let kills_self = (*radius_damage_affects
            & crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_KILLS_SELF)
            != 0
            && vid == *shooter_id;
        if !is_primary && !kills_self {
            let airborne = obj.is_significantly_above_terrain();
            let same_tmpl = crate::game_logic::weapon_bootstrap::splash_templates_equivalent(
                shooter_template,
                &obj.template_name,
            );
            let relationship = match players {
                Some(map) if team_factory.is_some() => {
                    crate::game_logic::GameLogic::object_relationship_from_owners(
                        team_factory.expect("guarded Some"),
                        map,
                        obj.owner_player_id,
                        &obj.team_instance_name,
                        *shooter_owner_player_id,
                        shooter_team_instance_name,
                    )
                }
                // C++ curVictim->getRelationship(source) is
                // ownership-driven; the live host falls back to
                // the frozen launch teams when no player
                // registry is wired into this combat pass.
                Some(_) if obj.owner_player_id == *shooter_owner_player_id => {
                    gamelogic::common::Relationship::Allies
                }
                Some(map) => {
                    let _ = map;
                    if obj.team == *shooter_team {
                        gamelogic::common::Relationship::Allies
                    } else {
                        gamelogic::common::Relationship::Neutral
                    }
                }
                None if obj.team == *shooter_team => gamelogic::common::Relationship::Allies,
                None => gamelogic::common::Relationship::Neutral,
            };
            let allowed = crate::game_logic::weapon_bootstrap::radius_damage_affects_victim(
                *radius_damage_affects,
                relationship,
                *shooter_id,
                vid,
                shooter_producer,
                airborne,
                same_tmpl,
            );
            if !allowed {
                return None;
            }
        }
        if !leftover_radius_damage_cone_allows(
            *radius_damage_angle,
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
                shooter_pos.unwrap_or(*position),
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
