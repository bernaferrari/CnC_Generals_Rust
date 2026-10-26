//! Original ParachuteContain death/collide callbacks, before base OpenContain.
use crate::game_logic::*;
use glam::Vec3;
impl GameLogic {
    fn remove_all_parachute_riders_owned(
        &mut self,
        id: ObjectId,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(object) = self.objects.get(&id) else {
            return Ok(());
        };
        let birth = std::sync::Arc::downgrade(&object.visual_draw_identity_token);
        loop {
            let Some(object) = self.objects.get(&id).filter(|object| {
                birth.ptr_eq(&std::sync::Arc::downgrade(
                    &object.visual_draw_identity_token,
                ))
            }) else {
                return Ok(());
            };
            let Some(rider) = object.occupants.first().copied() else {
                return Ok(());
            };
            let rider_birth = self
                .objects
                .get(&rider)
                .map(|object| std::sync::Arc::downgrade(&object.visual_draw_identity_token));
            self.remove_direct_contained_child_with_effects(id, rider, effects)?;
            if self.objects.get(&id).is_some_and(|object| {
                birth.ptr_eq(&std::sync::Arc::downgrade(
                    &object.visual_draw_identity_token,
                )) && object.occupants.first() == Some(&rider)
            }) && rider_birth.as_ref().is_some_and(|birth| {
                self.objects.get(&rider).is_some_and(|object| {
                    birth.ptr_eq(&std::sync::Arc::downgrade(
                        &object.visual_draw_identity_token,
                    ))
                })
            }) {
                return Err(gamelogic::GameLogicError::Configuration(
                    "ParachuteContain removeAllContained did not consume its receiving first rider"
                        .into(),
                ));
            }
        }
    }

    pub(in crate::game_logic) fn parachute_on_die_owned(
        &mut self,
        id: ObjectId,
        input: Option<crate::game_logic::object::OnDieInput>,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(object) = self.objects.get(&id) else {
            return Ok(());
        };
        let input = input.ok_or_else(|| {
            gamelogic::GameLogicError::Configuration(
                "ParachuteContain onDie requires the accepted incoming hit".into(),
            )
        })?;
        let position = object.get_position();
        let terrain = crate::game_logic::game_logic::FlightTerrainView {
            grid: &self.pathfinding_system.grid,
            logical: Some(self.world_services.terrain().as_ref()),
            terrain: self.terrain.as_ref(),
            samples: self.pathfinding_height_samples.as_ref(),
        };
        if !object.height_treats_as_airborne(
            position.y - terrain.raw_ground(position, object.ground_height),
        ) {
            return Ok(());
        }
        let Some(rider) = object.occupants.first().copied() else {
            return Ok(());
        };
        let Some(passenger) = self.objects.get(&rider) else {
            return Err(gamelogic::GameLogicError::InvalidObject(rider.0));
        };
        let birth = std::sync::Arc::downgrade(&object.visual_draw_identity_token);
        let rider_birth = std::sync::Arc::downgrade(&passenger.visual_draw_identity_token);
        let original = |world: &Self| {
            world.objects.get(&id).is_some_and(|object| {
                birth.ptr_eq(&std::sync::Arc::downgrade(
                    &object.visual_draw_identity_token,
                ))
            }) && world.objects.get(&rider).is_some_and(|object| {
                rider_birth.ptr_eq(&std::sync::Arc::downgrade(
                    &object.visual_draw_identity_token,
                ))
            })
        };
        self.remove_all_parachute_riders_owned(id, effects)?;
        if !original(self) {
            return Ok(());
        }
        let ratio = self
            .objects
            .get(&id)
            .unwrap()
            .thing()
            .template
            .contain_module
            .parachute
            .as_ref()
            .ok_or_else(|| {
                gamelogic::GameLogicError::Configuration(
                    "ParachuteContain onDie data missing".into(),
                )
            })?
            .free_fall_damage_percent;
        if ratio > 0. {
            // Source max health is reread AFTER removeAllContained callbacks.
            let amount = self.objects.get(&rider).unwrap().health.maximum * ratio;
            let context = DamageHitContext::new(
                input.source.and_then(|source| self.objects.get(&source)),
                None,
                combat::DamageType::Falling,
            );
            self.apply_owned_damage_with_effects(
                rider,
                amount,
                input.source,
                combat::DamageType::Falling,
                host_usa_pilot::HostDeathType::Splatted,
                None,
                &context,
                effects,
            );
        }
        if !original(self) {
            return Ok(());
        }
        let rider = self.objects.get_mut(&rider).unwrap();
        if rider
            .thing()
            .template
            .receiving_client_template
            .as_ref()
            .is_some_and(|template| template.has_behavior_module("PhysicsBehavior"))
        {
            rider.allow_to_fall = true;
            rider.is_in_freefall = true;
            rider.apply_physics_force(Vec3::ZERO);
        }
        Ok(())
    }
    pub(in crate::game_logic) fn parachute_on_ground_collide_owned(
        &mut self,
        id: ObjectId,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(object) = self.objects.get(&id) else {
            return Ok(());
        };
        if object.contained_by.is_some() {
            return Ok(());
        }
        let birth = std::sync::Arc::downgrade(&object.visual_draw_identity_token);
        self.remove_all_parachute_riders_owned(id, effects)?;
        if self.objects.get(&id).is_some_and(|object| {
            birth.ptr_eq(&std::sync::Arc::downgrade(
                &object.visual_draw_identity_token,
            ))
        }) {
            // Real Body kill starts actual SlowDeath, never direct destroy.
            self.apply_owned_kill_with_effects(
                id,
                combat::DamageType::Unresistable,
                host_usa_pilot::HostDeathType::Normal,
                effects,
            );
        }
        Ok(())
    }
}
